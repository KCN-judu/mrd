//! Exact boundary-native reference algorithms for ordinary rectilinear polygons.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::ops::Bound::{Excluded, Unbounded};
use std::time::Instant;

use rect_core::{
    Boundary, BoundaryIndex, BoundaryIndexError, BoundaryVertexId, CoordinateRect, DoubledPoint,
    FormalBoundaryIncidence, FormalRectilinearPolygon, GeometryError, HorizontalChord,
    HorizontalChordId, Point, PolygonError, PreparedPolygonContext, RectilinearPolygon,
    VerticalChord, VerticalChordId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::EffectiveChordFamilies;
use crate::polygon_arrangement;
use crate::polygon_cut_index;
use crate::polygon_sparse::{
    self, PolygonDissectionValidatorBackend, PolygonRecoveryBackend, SparseOrthogonalSubdivision,
    SparseSlabMetrics, SparseSlabValidator, SparseSubdivisionMetrics, SparseValidatorBackend,
};
use crate::{
    ChordRef, CleanHoleFreeCertificate, CleanRejectionReason, EffectiveChordEndpointIndex,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct GeneralPolygonPairwiseEnumerator;

/// Explicit v0.9 all-reflex-pairs and full-boundary-scan reference backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReferencePolygonPairwiseEnumerator;

/// Aligned-pair enumerator backed by one prepared exact orthogonal edge index.
#[derive(Clone, Copy, Debug, Default)]
pub struct IndexedPolygonPairwiseEnumerator;

/// Source-mapped event sweep for Definition 7 chords on ordinary polygons.
#[derive(Clone, Copy, Debug, Default)]
pub struct SoltanGorpinevichSweepEnumerator;

/// Axis selected by the ordinary-polygon sweep.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SweepAxis {
    Horizontal,
    Vertical,
}

impl SweepAxis {
    const fn name(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Bounded diagnostic summary for one scan coordinate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SweepEventSummary {
    pub axis: SweepAxis,
    pub coordinate: i64,
    pub inserted_segment_count: usize,
    pub query_count: usize,
    pub removed_segment_count: usize,
    /// The sweep processes every equal-coordinate bucket as insert, query,
    /// remove, which implements the required closed-event convention.
    pub insert_query_remove_order: bool,
}

/// Provenance for one canonical sweep output record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SweepOutputRecord {
    pub axis: SweepAxis,
    pub source: BoundaryVertexId,
    pub target: BoundaryVertexId,
    pub source_point: Point,
    pub target_point: Point,
    pub blocker_edge_id: usize,
}

/// Audit metadata for the source-mapped ordinary-polygon sweep.
///
/// Output provenance is output-sized, while the event trace is always bounded.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SweepCertificate {
    pub output_records: Vec<SweepOutputRecord>,
    pub event_summaries: Vec<SweepEventSummary>,
    pub event_trace_truncated: bool,
}

const SWEEP_EVENT_TRACE_LIMIT: usize = 64;

type SweepChordKeys = BTreeSet<(i64, i64, i64)>;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonChordEnumerationMetrics {
    pub polygon_boundary_edge_visits: usize,
    pub polygon_point_location_queries: usize,
    pub polygon_segment_reporting_queries: usize,
    pub polygon_reported_boundary_intersections: usize,
    pub polygon_aligned_reflex_candidate_pairs: usize,
    pub polygon_unaligned_reflex_pair_checks: usize,
    pub polygon_definition7_full_boundary_scans: usize,
    pub sweep_horizontal_event_count: usize,
    pub sweep_vertical_event_count: usize,
    pub sweep_status_insertions: usize,
    pub sweep_status_deletions: usize,
    pub sweep_status_queries: usize,
    pub sweep_auxiliary_tree_operations: usize,
    pub sweep_output_horizontal_chords: usize,
    pub sweep_output_vertical_chords: usize,
    pub sweep_duplicate_output_count: usize,
    pub sweep_aligned_pair_iterations: usize,
    pub sweep_all_pair_iterations: usize,
    pub sweep_definition7_fallback_checks: usize,
    pub sweep_full_boundary_scans: usize,
    pub sweep_horizontal_microseconds: u128,
    pub sweep_vertical_microseconds: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolygonChordEnumerationResult {
    pub families: EffectiveChordFamilies,
    pub metrics: PolygonChordEnumerationMetrics,
    pub sweep_certificate: Option<SweepCertificate>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HorizontalCutSegment {
    pub left: i64,
    pub right: i64,
    pub y: i64,
}

impl HorizontalCutSegment {
    pub(crate) fn new(left: i64, right: i64, y: i64) -> Result<Self, PolygonSgError> {
        if left >= right {
            return Err(PolygonSgError::InvalidSimpleChord {
                start: Point::new(left, y),
            });
        }
        Ok(Self { left, right, y })
    }

    fn from_chord(chord: HorizontalChord) -> Self {
        Self {
            left: chord.left(),
            right: chord.right(),
            y: chord.y(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VerticalCutSegment {
    pub x: i64,
    pub bottom: i64,
    pub top: i64,
}

impl VerticalCutSegment {
    pub(crate) fn new(x: i64, bottom: i64, top: i64) -> Result<Self, PolygonSgError> {
        if bottom >= top {
            return Err(PolygonSgError::InvalidSimpleChord {
                start: Point::new(x, bottom),
            });
        }
        Ok(Self { x, bottom, top })
    }

    fn from_chord(chord: VerticalChord) -> Self {
        Self {
            x: chord.x(),
            bottom: chord.bottom(),
            top: chord.top(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonCompletionMetrics {
    pub selected_cut_materialization_microseconds: u128,
    pub horizontal_completion_microseconds: u128,
    pub vertical_completion_microseconds: u128,
    pub rectangle_recovery_microseconds: u128,
    pub final_validation_microseconds: u128,
    pub horizontal_candidate_queries: usize,
    pub vertical_candidate_queries: usize,
    pub horizontal_simple_chord_count: usize,
    pub vertical_simple_chord_count: usize,
    pub coordinate_compression_x_count: usize,
    pub coordinate_compression_y_count: usize,
    pub atomic_cell_count: usize,
    pub rectangle_recovery_visits: usize,
    pub completion_global_candidate_rebuilds: usize,
    pub completion_cut_pair_tests: usize,
    pub completion_intersections_reported: usize,
    pub completion_candidate_insertions: usize,
    pub completion_candidate_revalidations: usize,
    pub completion_stale_candidates: usize,
    pub completion_boundary_ray_queries: usize,
    pub completion_cut_ray_queries: usize,
    pub completion_full_boundary_scans: usize,
    pub completion_full_cut_scans: usize,
    pub arrangement_point_location_queries: usize,
    pub arrangement_boundary_edge_visits: usize,
    pub arrangement_span_writes: usize,
    pub polygon_validator_rectangle_cell_tests: usize,
    pub arrangement_owned_bytes: usize,
    pub cut_index: polygon_cut_index::Metrics,
    pub sparse_subdivision_vertices: usize,
    pub sparse_subdivision_half_edges: usize,
    pub sparse_subdivision_faces: usize,
    pub sparse_subdivision_junctions: usize,
    pub sparse_subdivision_owned_bytes: usize,
    pub sparse_validator_slab_count: usize,
    pub sparse_subdivision: SparseSubdivisionMetrics,
    pub sparse_validator: SparseSlabMetrics,
    pub recovery_policy: String,
    pub selected_recovery_backend: String,
    pub dense_recovery_retained_byte_estimate: usize,
    pub sparse_recovery_retained_upper_estimate: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonCompletionResult {
    pub rectangles: Vec<CoordinateRect>,
    pub selected_horizontal_cuts: Vec<HorizontalCutSegment>,
    pub selected_vertical_cuts: Vec<VerticalCutSegment>,
    pub added_horizontal_cuts: Vec<HorizontalCutSegment>,
    pub added_vertical_cuts: Vec<VerticalCutSegment>,
    pub metrics: PolygonCompletionMetrics,
}

/// Incremental indexed polygon completion backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct IndexedPolygonCompletion;

#[derive(Clone, Copy, Debug, Default)]
pub struct CoordinateCompressedCompletion;

struct FormalCompletionInputs {
    incidence: FormalBoundaryIncidence,
    formal_points: BTreeSet<Point>,
    selected_horizontal: Vec<HorizontalCutSegment>,
    selected_vertical: Vec<VerticalCutSegment>,
    horizontal: BTreeSet<HorizontalCutSegment>,
    vertical: BTreeSet<VerticalCutSegment>,
}

struct FormalRecovery {
    dense: PolygonRecovery,
    subdivision: SparseSubdivisionMetrics,
    sparse_validation: SparseSlabMetrics,
}

impl CoordinateCompressedCompletion {
    /// Completes a formal polygon with the source Step 3--4 policy.
    ///
    /// Ornament segments are initial boundary barriers and all formal vertices
    /// are completion candidates. Dense and sparse recovery must produce the
    /// same canonical rectangles before Definition 2 validation succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid selections, incomplete rays,
    /// backend disagreement, or incomplete formal-boundary coverage.
    pub fn complete_formal(
        &self,
        polygon: &FormalRectilinearPolygon,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        if horizontal_chords.len() != selected_horizontal.len()
            || vertical_chords.len() != selected_vertical.len()
        {
            return Err(PolygonSgError::SelectionLengthMismatch);
        }
        let prepared =
            PreparedPolygonContext::new(polygon.region()).map_err(|error| match error {
                rect_core::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
                rect_core::PreparedPolygonError::BoundaryIndex(error) => {
                    PolygonSgError::BoundaryIndex(error)
                }
            })?;
        let mut inputs = prepare_formal_completion(
            polygon,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
        )?;
        let mut added_horizontal_cuts = Vec::new();
        let mut added_vertical_cuts = Vec::new();
        let mut metrics = PolygonCompletionMetrics::default();
        complete_polygon_axis(
            polygon.region(),
            &inputs.formal_points,
            &mut inputs.horizontal,
            &mut inputs.vertical,
            true,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;
        complete_polygon_axis(
            polygon.region(),
            &inputs.formal_points,
            &mut inputs.horizontal,
            &mut inputs.vertical,
            false,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;
        added_horizontal_cuts = normalize_horizontal_segments(added_horizontal_cuts);
        added_vertical_cuts = normalize_vertical_segments(added_vertical_cuts);
        let recovery = recover_and_validate_formal(
            polygon,
            &prepared,
            &inputs.incidence,
            &inputs.horizontal,
            &inputs.vertical,
        )?;
        metrics.coordinate_compression_x_count = recovery.dense.x_count;
        metrics.coordinate_compression_y_count = recovery.dense.y_count;
        metrics.atomic_cell_count = recovery.dense.atomic_cell_count;
        metrics.rectangle_recovery_visits = recovery.dense.visits;
        metrics.sparse_subdivision_vertices = recovery.subdivision.vertex_count;
        metrics.sparse_subdivision_half_edges = recovery.subdivision.half_edge_count;
        metrics.sparse_subdivision_faces = recovery.subdivision.face_count;
        metrics.sparse_subdivision_junctions = recovery.subdivision.junction_count;
        metrics.sparse_subdivision_owned_bytes = recovery.subdivision.owned_bytes;
        metrics.sparse_subdivision = recovery.subdivision;
        metrics.sparse_validator_slab_count = recovery.sparse_validation.slab_count;
        metrics.sparse_validator = recovery.sparse_validation;
        Ok(PolygonCompletionResult {
            rectangles: recovery.dense.rectangles,
            selected_horizontal_cuts: inputs.selected_horizontal,
            selected_vertical_cuts: inputs.selected_vertical,
            added_horizontal_cuts,
            added_vertical_cuts,
            metrics,
        })
    }

    /// Completes a selected admissible effective-chord family into rectangles.
    ///
    /// The reference policy inserts selected chords, then horizontal simple
    /// chords, then vertical simple chords. Rectangle recovery is sensitive to
    /// the coordinate arrangement, not to coordinate magnitude.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid selections, incomplete rays, or
    /// nonrectangular recovered regions.
    pub fn complete(
        &self,
        polygon: &RectilinearPolygon,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            rect_core::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            rect_core::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        self.complete_prepared(
            &prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
        )
    }

    /// Runs the preserved completion policy on one shared polygon context.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid selections, incomplete rays, or
    /// nonrectangular recovered regions.
    pub fn complete_prepared(
        &self,
        prepared: &PreparedPolygonContext,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        let started = Instant::now();
        if horizontal_chords.len() != selected_horizontal.len()
            || vertical_chords.len() != selected_vertical.len()
        {
            return Err(PolygonSgError::SelectionLengthMismatch);
        }
        let polygon = prepared.polygon();
        let mut selected_horizontal_cuts = horizontal_chords
            .iter()
            .zip(selected_horizontal)
            .filter_map(|(&chord, &selected)| {
                selected.then_some(HorizontalCutSegment::from_chord(chord))
            })
            .collect::<Vec<_>>();
        let mut selected_vertical_cuts = vertical_chords
            .iter()
            .zip(selected_vertical)
            .filter_map(|(&chord, &selected)| {
                selected.then_some(VerticalCutSegment::from_chord(chord))
            })
            .collect::<Vec<_>>();
        selected_horizontal_cuts = normalize_horizontal_segments(selected_horizontal_cuts);
        selected_vertical_cuts = normalize_vertical_segments(selected_vertical_cuts);
        let mut horizontal_cuts = selected_horizontal_cuts
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut vertical_cuts = selected_vertical_cuts
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut added_horizontal_cuts = Vec::new();
        let mut added_vertical_cuts = Vec::new();
        let selected_at = Instant::now();
        let mut metrics = PolygonCompletionMetrics {
            selected_cut_materialization_microseconds: selected_at
                .duration_since(started)
                .as_micros(),
            ..PolygonCompletionMetrics::default()
        };

        complete_polygon_axis(
            polygon,
            &BTreeSet::new(),
            &mut horizontal_cuts,
            &mut vertical_cuts,
            true,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;
        let horizontal_at = Instant::now();
        metrics.horizontal_completion_microseconds =
            horizontal_at.duration_since(selected_at).as_micros();
        complete_polygon_axis(
            polygon,
            &BTreeSet::new(),
            &mut horizontal_cuts,
            &mut vertical_cuts,
            false,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;
        let vertical_at = Instant::now();
        metrics.vertical_completion_microseconds =
            vertical_at.duration_since(horizontal_at).as_micros();

        added_horizontal_cuts = normalize_horizontal_segments(added_horizontal_cuts);
        added_vertical_cuts = normalize_vertical_segments(added_vertical_cuts);

        let recovery = recover_coordinate_rectangles(polygon, &horizontal_cuts, &vertical_cuts)?;
        let recovered_at = Instant::now();
        metrics.rectangle_recovery_microseconds =
            recovered_at.duration_since(vertical_at).as_micros();
        metrics.coordinate_compression_x_count = recovery.x_count;
        metrics.coordinate_compression_y_count = recovery.y_count;
        metrics.atomic_cell_count = recovery.atomic_cell_count;
        metrics.rectangle_recovery_visits = recovery.visits;
        validate_polygon_dissection(polygon, &recovery.rectangles)?;
        metrics.final_validation_microseconds = recovered_at.elapsed().as_micros();
        Ok(PolygonCompletionResult {
            rectangles: recovery.rectangles,
            selected_horizontal_cuts,
            selected_vertical_cuts,
            added_horizontal_cuts,
            added_vertical_cuts,
            metrics,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "coordinate-compressed"
    }
}

fn prepare_formal_completion(
    polygon: &FormalRectilinearPolygon,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
) -> Result<FormalCompletionInputs, PolygonSgError> {
    let incidence = polygon
        .incidence()
        .map_err(|error| PolygonSgError::Formal {
            message: error.to_string(),
        })?;
    let formal_points = incidence
        .vertices
        .iter()
        .map(|vertex| vertex.point)
        .collect::<BTreeSet<_>>();
    let selected_horizontal = normalize_horizontal_segments(
        horizontal_chords
            .iter()
            .zip(selected_horizontal)
            .filter_map(|(&chord, &selected)| {
                selected.then_some(HorizontalCutSegment::from_chord(chord))
            })
            .collect(),
    );
    let selected_vertical = normalize_vertical_segments(
        vertical_chords
            .iter()
            .zip(selected_vertical)
            .filter_map(|(&chord, &selected)| {
                selected.then_some(VerticalCutSegment::from_chord(chord))
            })
            .collect(),
    );
    let mut horizontal = selected_horizontal.iter().copied().collect::<BTreeSet<_>>();
    let mut vertical = selected_vertical.iter().copied().collect::<BTreeSet<_>>();
    for segment in &incidence.elementary_segments {
        if !segment
            .sources
            .iter()
            .any(|source| matches!(source, rect_core::FormalBoundarySource::Ornament { .. }))
        {
            continue;
        }
        let first = incidence.vertices[segment.start.0].point;
        let second = incidence.vertices[segment.end.0].point;
        if first.y == second.y {
            horizontal.insert(HorizontalCutSegment::new(
                first.x.min(second.x),
                first.x.max(second.x),
                first.y,
            )?);
        } else {
            vertical.insert(VerticalCutSegment::new(
                first.x,
                first.y.min(second.y),
                first.y.max(second.y),
            )?);
        }
    }
    Ok(FormalCompletionInputs {
        incidence,
        formal_points,
        selected_horizontal,
        selected_vertical,
        horizontal,
        vertical,
    })
}

fn recover_and_validate_formal(
    polygon: &FormalRectilinearPolygon,
    prepared: &PreparedPolygonContext,
    incidence: &FormalBoundaryIncidence,
    horizontal: &BTreeSet<HorizontalCutSegment>,
    vertical: &BTreeSet<VerticalCutSegment>,
) -> Result<FormalRecovery, PolygonSgError> {
    let dense = recover_coordinate_rectangles(polygon.region(), horizontal, vertical)?;
    let sparse = SparseOrthogonalSubdivision::new(prepared, horizontal, vertical)?;
    let sparse_rectangles = sparse.recover_rectangles(polygon.region())?;
    if dense.rectangles != sparse_rectangles {
        return Err(PolygonSgError::FormalRecoveryMismatch);
    }
    validate_polygon_dissection(polygon.region(), &dense.rectangles)?;
    SparseSlabValidator.validate_with_backend(
        polygon.region(),
        &dense.rectangles,
        SparseValidatorBackend::ReferenceSlabRescan,
    )?;
    let sparse_validation = SparseSlabValidator.validate_with_backend(
        polygon.region(),
        &dense.rectangles,
        SparseValidatorBackend::EventSegmentTree,
    )?;
    validate_formal_boundary_coverage(incidence, &dense.rectangles)?;
    Ok(FormalRecovery {
        dense,
        subdivision: sparse.metrics,
        sparse_validation,
    })
}

fn normalize_horizontal_segments(
    mut segments: Vec<HorizontalCutSegment>,
) -> Vec<HorizontalCutSegment> {
    segments.sort_unstable_by_key(|segment| (segment.y, segment.left, segment.right));
    let mut normalized = Vec::<HorizontalCutSegment>::new();
    for segment in segments {
        if let Some(last) = normalized.last_mut()
            && last.y == segment.y
            && segment.left <= last.right
        {
            last.right = last.right.max(segment.right);
        } else {
            normalized.push(segment);
        }
    }
    normalized.sort_unstable();
    normalized
}

fn normalize_vertical_segments(mut segments: Vec<VerticalCutSegment>) -> Vec<VerticalCutSegment> {
    segments.sort_unstable_by_key(|segment| (segment.x, segment.bottom, segment.top));
    let mut normalized = Vec::<VerticalCutSegment>::new();
    for segment in segments {
        if let Some(last) = normalized.last_mut()
            && last.x == segment.x
            && segment.bottom <= last.top
        {
            last.top = last.top.max(segment.top);
        } else {
            normalized.push(segment);
        }
    }
    normalized.sort_unstable();
    normalized
}

fn completion_coordinate_universe(
    prepared: &PreparedPolygonContext,
    selected_horizontal: &[HorizontalCutSegment],
    selected_vertical: &[VerticalCutSegment],
) -> BTreeSet<i64> {
    let mut coordinates = prepared
        .polygon()
        .loops()
        .flat_map(|boundary_loop| {
            boundary_loop
                .vertices
                .iter()
                .flat_map(|point| [point.x, point.y])
        })
        .collect::<BTreeSet<_>>();
    for cut in selected_horizontal {
        coordinates.extend([cut.left, cut.right, cut.y]);
    }
    for cut in selected_vertical {
        coordinates.extend([cut.x, cut.bottom, cut.top]);
    }
    coordinates
}

fn completion_coordinate_axis_counts(
    prepared: &PreparedPolygonContext,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
) -> (usize, usize) {
    let mut xs = prepared
        .polygon()
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
        .collect::<BTreeSet<_>>();
    let mut ys = prepared
        .polygon()
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
        .collect::<BTreeSet<_>>();
    for cut in horizontal_cuts {
        xs.extend([cut.left, cut.right]);
        ys.insert(cut.y);
    }
    for cut in vertical_cuts {
        xs.insert(cut.x);
        ys.extend([cut.bottom, cut.top]);
    }
    (xs.len(), ys.len())
}

impl GeneralPolygonPairwiseEnumerator {
    /// Enumerates every Definition 7 effective chord for an ordinary polygon.
    ///
    /// This is an exact `O(r^2 n)` reference implementation, not the general
    /// Soltan--Gorpinevich sweep-line algorithm.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] when normalization, boundary indexing, or
    /// exact chord construction fails.
    pub fn enumerate(
        &self,
        polygon: &RectilinearPolygon,
    ) -> Result<EffectiveChordFamilies, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            rect_core::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            rect_core::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        self.enumerate_prepared(&prepared)
    }

    /// Runs the preserved full-pair reference algorithm on shared metadata.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for endpoint metadata or chord geometry
    /// failures.
    pub fn enumerate_prepared(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<EffectiveChordFamilies, PolygonSgError> {
        Ok(self.enumerate_prepared_with_metrics(prepared)?.families)
    }

    /// Runs the preserved pairwise reference algorithm and returns its scan
    /// counters as well as the exact chord families.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid endpoint metadata, coordinate
    /// overflow, or effective-chord construction failure.
    pub fn enumerate_prepared_with_metrics(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<PolygonChordEnumerationResult, PolygonSgError> {
        let polygon = prepared.polygon();
        let boundary = prepared.boundary();
        let boundary_index = prepared.boundary_index();
        let points = boundary
            .reflex_vertices
            .iter()
            .map(|vertex| vertex.point)
            .collect::<Vec<_>>();
        let mut horizontal = BTreeSet::new();
        let mut vertical = BTreeSet::new();
        let total_pairs = points.len().saturating_sub(1) * points.len() / 2;
        let aligned_pairs = prepared.metrics().polygon_aligned_reflex_candidate_pairs;
        let mut metrics = PolygonChordEnumerationMetrics {
            polygon_aligned_reflex_candidate_pairs: aligned_pairs,
            polygon_unaligned_reflex_pair_checks: total_pairs.saturating_sub(aligned_pairs),
            ..PolygonChordEnumerationMetrics::default()
        };

        for first_index in 0..points.len() {
            for second_index in first_index + 1..points.len() {
                let first = points[first_index];
                let second = points[second_index];
                if first.y == second.y {
                    let left = first.x.min(second.x);
                    let right = first.x.max(second.x);
                    if endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(left, first.y),
                        true,
                    )? && endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(right, first.y),
                        true,
                    )? && horizontal_satisfies_definition_7(
                        polygon,
                        boundary,
                        boundary_index,
                        left,
                        right,
                        first.y,
                        &mut metrics,
                    )? {
                        horizontal.insert((first.y, left, right));
                    }
                }
                if first.x == second.x {
                    let bottom = first.y.min(second.y);
                    let top = first.y.max(second.y);
                    if endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(first.x, bottom),
                        false,
                    )? && endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(first.x, top),
                        false,
                    )? && vertical_satisfies_definition_7(
                        polygon,
                        boundary,
                        boundary_index,
                        first.x,
                        bottom,
                        top,
                        &mut metrics,
                    )? {
                        vertical.insert((first.x, bottom, top));
                    }
                }
            }
        }

        let families = EffectiveChordFamilies {
            horizontal: horizontal
                .into_iter()
                .enumerate()
                .map(|(index, (y, left, right))| {
                    HorizontalChord::new(HorizontalChordId(index), left, right, y)
                })
                .collect::<Result<Vec<_>, _>>()?,
            vertical: vertical
                .into_iter()
                .enumerate()
                .map(|(index, (x, bottom, top))| {
                    VerticalChord::new(VerticalChordId(index), x, bottom, top)
                })
                .collect::<Result<Vec<_>, _>>()?,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(total_pairs),
        };
        Ok(PolygonChordEnumerationResult {
            families,
            metrics,
            sweep_certificate: None,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "general-polygon-pairwise"
    }
}

impl ReferencePolygonPairwiseEnumerator {
    /// Runs the preserved v0.9 reference enumerator.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] under the same contract as
    /// [`GeneralPolygonPairwiseEnumerator::enumerate`].
    pub fn enumerate(
        &self,
        polygon: &RectilinearPolygon,
    ) -> Result<EffectiveChordFamilies, PolygonSgError> {
        GeneralPolygonPairwiseEnumerator.enumerate(polygon)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "reference-polygon-pairwise"
    }
}

impl IndexedPolygonPairwiseEnumerator {
    /// Convenience API that prepares the polygon once before indexed enumeration.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid polygon metadata or chord geometry.
    pub fn enumerate(
        &self,
        polygon: &RectilinearPolygon,
    ) -> Result<EffectiveChordFamilies, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            rect_core::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            rect_core::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        Ok(self.enumerate_prepared(&prepared)?.families)
    }

    /// Enumerates only coordinate-aligned reflex pairs using prepared indexes.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] when endpoint metadata or exact chord
    /// construction fails.
    pub fn enumerate_prepared(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<PolygonChordEnumerationResult, PolygonSgError> {
        let mut horizontal = BTreeSet::new();
        let mut vertical = BTreeSet::new();
        let mut metrics = PolygonChordEnumerationMetrics {
            polygon_aligned_reflex_candidate_pairs: prepared
                .metrics()
                .polygon_aligned_reflex_candidate_pairs,
            ..PolygonChordEnumerationMetrics::default()
        };

        for points in prepared.reflex_by_y().values() {
            for first in 0..points.len() {
                for second in first + 1..points.len() {
                    let left = points[first].x.min(points[second].x);
                    let right = points[first].x.max(points[second].x);
                    let y = points[first].y;
                    if endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(left, y),
                        true,
                    )? && endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(right, y),
                        true,
                    )? && horizontal_satisfies_definition_7_indexed(
                        prepared,
                        left,
                        right,
                        y,
                        &mut metrics,
                    )? {
                        horizontal.insert((y, left, right));
                    }
                }
            }
        }
        for points in prepared.reflex_by_x().values() {
            for first in 0..points.len() {
                for second in first + 1..points.len() {
                    let x = points[first].x;
                    let bottom = points[first].y.min(points[second].y);
                    let top = points[first].y.max(points[second].y);
                    if endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(x, bottom),
                        false,
                    )? && endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(x, top),
                        false,
                    )? && vertical_satisfies_definition_7_indexed(
                        prepared,
                        x,
                        bottom,
                        top,
                        &mut metrics,
                    )? {
                        vertical.insert((x, bottom, top));
                    }
                }
            }
        }

        let families = EffectiveChordFamilies {
            horizontal: horizontal
                .into_iter()
                .enumerate()
                .map(|(index, (y, left, right))| {
                    HorizontalChord::new(HorizontalChordId(index), left, right, y)
                })
                .collect::<Result<Vec<_>, _>>()?,
            vertical: vertical
                .into_iter()
                .enumerate()
                .map(|(index, (x, bottom, top))| {
                    VerticalChord::new(VerticalChordId(index), x, bottom, top)
                })
                .collect::<Result<Vec<_>, _>>()?,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(metrics.polygon_aligned_reflex_candidate_pairs),
        };
        Ok(PolygonChordEnumerationResult {
            families,
            metrics,
            sweep_certificate: None,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "indexed-polygon-pairwise"
    }
}

#[derive(Default)]
struct SweepEventBucket {
    insertions: Vec<usize>,
    removals: Vec<usize>,
    queries: Vec<(Point, BoundaryVertexId)>,
}

impl SoltanGorpinevichSweepEnumerator {
    /// Enumerates effective chords with the source-mapped ordinary-polygon
    /// event sweep.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid prepared boundary metadata or
    /// chord-coordinate construction.
    pub fn enumerate(
        &self,
        polygon: &RectilinearPolygon,
    ) -> Result<EffectiveChordFamilies, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            rect_core::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            rect_core::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        Ok(self.enumerate_prepared(&prepared)?.families)
    }

    /// Runs the axis-generic source-mapped event sweep on shared metadata.
    ///
    /// The accepted ordinary-loop model makes Definition 7's formal-boundary
    /// merge cases inapplicable; see `docs/SOLTAN_SWEEP_IMPLEMENTATION.md`.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid boundary identities or chord
    /// construction failure.
    pub fn enumerate_prepared(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<PolygonChordEnumerationResult, PolygonSgError> {
        let mut metrics = PolygonChordEnumerationMetrics::default();
        let horizontal_started = Instant::now();
        let (horizontal, mut certificate) =
            enumerate_sweep_axis(prepared, SweepAxis::Horizontal, &mut metrics)?;
        metrics.sweep_horizontal_microseconds = horizontal_started.elapsed().as_micros();
        let vertical_started = Instant::now();
        let (vertical, vertical_certificate) =
            enumerate_sweep_axis(prepared, SweepAxis::Vertical, &mut metrics)?;
        metrics.sweep_vertical_microseconds = vertical_started.elapsed().as_micros();
        certificate
            .output_records
            .extend(vertical_certificate.output_records);
        certificate
            .event_summaries
            .extend(vertical_certificate.event_summaries);
        certificate.event_trace_truncated |= vertical_certificate.event_trace_truncated;
        certificate.output_records.sort_unstable_by_key(|record| {
            (
                record.axis.name(),
                record.source_point,
                record.target_point,
                record.source,
                record.target,
            )
        });
        certificate
            .event_summaries
            .sort_unstable_by_key(|summary| (summary.axis.name(), summary.coordinate));

        let families = EffectiveChordFamilies {
            horizontal: horizontal
                .into_iter()
                .enumerate()
                .map(|(index, (y, left, right))| {
                    HorizontalChord::new(HorizontalChordId(index), left, right, y)
                })
                .collect::<Result<Vec<_>, _>>()?,
            vertical: vertical
                .into_iter()
                .enumerate()
                .map(|(index, (x, bottom, top))| {
                    VerticalChord::new(VerticalChordId(index), x, bottom, top)
                })
                .collect::<Result<Vec<_>, _>>()?,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(0),
        };
        Ok(PolygonChordEnumerationResult {
            families,
            metrics,
            sweep_certificate: Some(certificate),
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "sg-sweep"
    }
}

/// Fully audits ordinary-loop sweep output against Definition 7 and the
/// preserved pairwise oracle.
///
/// # Errors
///
/// Returns [`PolygonSgError::SweepAuditFailed`] when a provenance record,
/// nearest blocker, event bucket, or reference chord is inconsistent.
pub fn audit_sweep_provenance(
    prepared: &PreparedPolygonContext,
    result: &PolygonChordEnumerationResult,
) -> Result<(), PolygonSgError> {
    let certificate =
        result
            .sweep_certificate
            .as_ref()
            .ok_or_else(|| PolygonSgError::SweepAuditFailed {
                message: "sweep result omitted its certificate".to_owned(),
            })?;
    let reference = GeneralPolygonPairwiseEnumerator.enumerate_prepared(prepared)?;
    let expected = reference_chord_keys(&reference);
    let recorded = certificate
        .output_records
        .iter()
        .map(sweep_record_key)
        .collect::<BTreeSet<_>>();
    if recorded != expected || sweep_family_keys(&result.families) != expected {
        return Err(PolygonSgError::SweepAuditFailed {
            message: "reference effective chord lacks matching first-hit provenance".to_owned(),
        });
    }
    let mut metrics = PolygonChordEnumerationMetrics::default();
    for record in &certificate.output_records {
        let (low, high, fixed) = match record.axis {
            SweepAxis::Horizontal => (
                record.source_point.x.min(record.target_point.x),
                record.source_point.x.max(record.target_point.x),
                record.source_point.y,
            ),
            SweepAxis::Vertical => (
                record.source_point.y.min(record.target_point.y),
                record.source_point.y.max(record.target_point.y),
                record.source_point.x,
            ),
        };
        let definition_7 = match record.axis {
            SweepAxis::Horizontal => {
                horizontal_satisfies_definition_7_indexed(prepared, low, high, fixed, &mut metrics)?
            }
            SweepAxis::Vertical => {
                vertical_satisfies_definition_7_indexed(prepared, fixed, low, high, &mut metrics)?
            }
        };
        if !definition_7 {
            return Err(PolygonSgError::SweepAuditFailed {
                message: "sweep output violates Definition 7".to_owned(),
            });
        }
        if prepared.boundary_index().vertex_id(record.source_point) != Some(record.source)
            || prepared.boundary_index().vertex_id(record.target_point) != Some(record.target)
        {
            return Err(PolygonSgError::SweepAuditFailed {
                message: "sweep record boundary identities are stale".to_owned(),
            });
        }
        let increasing = sweep_interior_direction(prepared.boundary(), record.source, record.axis)?;
        let direction = match (record.axis, increasing) {
            (SweepAxis::Horizontal, true) => rect_core::OrthogonalDirection::East,
            (SweepAxis::Horizontal, false) => rect_core::OrthogonalDirection::West,
            (SweepAxis::Vertical, true) => rect_core::OrthogonalDirection::North,
            (SweepAxis::Vertical, false) => rect_core::OrthogonalDirection::South,
        };
        if prepared
            .edge_index()
            .nearest_boundary_blocker(record.source_point, direction)
            != Some(record.target_point)
        {
            return Err(PolygonSgError::SweepAuditFailed {
                message: "recorded sweep blocker is not the nearest boundary blocker".to_owned(),
            });
        }
        let blocker = prepared
            .edge_index()
            .edge(record.blocker_edge_id)
            .ok_or_else(|| PolygonSgError::SweepAuditFailed {
                message: "recorded blocker edge is absent".to_owned(),
            })?;
        let blocker_valid = match record.axis {
            SweepAxis::Horizontal => {
                !blocker.is_horizontal()
                    && blocker.first.x == record.target_point.x
                    && blocker.bottom() <= record.target_point.y
                    && record.target_point.y <= blocker.top()
            }
            SweepAxis::Vertical => {
                blocker.is_horizontal()
                    && blocker.first.y == record.target_point.y
                    && blocker.left() <= record.target_point.x
                    && record.target_point.x <= blocker.right()
            }
        };
        if !blocker_valid {
            return Err(PolygonSgError::SweepAuditFailed {
                message: "recorded blocker edge does not realize the target".to_owned(),
            });
        }
    }
    audit_sweep_event_buckets(prepared, certificate)?;
    Ok(())
}

fn reference_chord_keys(families: &EffectiveChordFamilies) -> BTreeSet<(SweepAxis, i64, i64, i64)> {
    families
        .horizontal
        .iter()
        .map(|chord| {
            (
                SweepAxis::Horizontal,
                chord.y(),
                chord.left(),
                chord.right(),
            )
        })
        .chain(
            families
                .vertical
                .iter()
                .map(|chord| (SweepAxis::Vertical, chord.x(), chord.bottom(), chord.top())),
        )
        .collect()
}

fn sweep_family_keys(families: &EffectiveChordFamilies) -> BTreeSet<(SweepAxis, i64, i64, i64)> {
    reference_chord_keys(families)
}

fn sweep_record_key(record: &SweepOutputRecord) -> (SweepAxis, i64, i64, i64) {
    match record.axis {
        SweepAxis::Horizontal => (
            record.axis,
            record.source_point.y,
            record.source_point.x.min(record.target_point.x),
            record.source_point.x.max(record.target_point.x),
        ),
        SweepAxis::Vertical => (
            record.axis,
            record.source_point.x,
            record.source_point.y.min(record.target_point.y),
            record.source_point.y.max(record.target_point.y),
        ),
    }
}

fn audit_sweep_event_buckets(
    prepared: &PreparedPolygonContext,
    certificate: &SweepCertificate,
) -> Result<(), PolygonSgError> {
    for axis in [SweepAxis::Horizontal, SweepAxis::Vertical] {
        let mut expected = BTreeMap::<i64, (usize, usize, usize)>::new();
        for edge_id in 0..prepared.edge_index().edge_count() {
            let edge = prepared
                .edge_index()
                .edge(edge_id)
                .expect("edge identity is indexed");
            let status = match axis {
                SweepAxis::Horizontal => !edge.is_horizontal(),
                SweepAxis::Vertical => edge.is_horizontal(),
            };
            if status {
                let (start, end) = match axis {
                    SweepAxis::Horizontal => (edge.bottom(), edge.top()),
                    SweepAxis::Vertical => (edge.left(), edge.right()),
                };
                expected.entry(start).or_default().0 += 1;
                expected.entry(end).or_default().2 += 1;
            }
        }
        for vertex in &prepared.boundary().reflex_vertices {
            let coordinate = match axis {
                SweepAxis::Horizontal => vertex.point.y,
                SweepAxis::Vertical => vertex.point.x,
            };
            expected.entry(coordinate).or_default().1 += 1;
        }
        for (coordinate, (insertions, queries, removals)) in expected {
            if let Some(summary) = certificate
                .event_summaries
                .iter()
                .find(|summary| summary.axis == axis && summary.coordinate == coordinate)
                && (summary.inserted_segment_count != insertions
                    || summary.query_count != queries
                    || summary.removed_segment_count != removals
                    || !summary.insert_query_remove_order)
            {
                return Err(PolygonSgError::SweepAuditFailed {
                    message: "closed-event tie summary disagrees with the boundary".to_owned(),
                });
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn enumerate_sweep_axis(
    prepared: &PreparedPolygonContext,
    axis: SweepAxis,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<(SweepChordKeys, SweepCertificate), PolygonSgError> {
    let boundary = prepared.boundary();
    let boundary_index = prepared.boundary_index();
    let edge_index = prepared.edge_index();
    let mut events = BTreeMap::<i64, SweepEventBucket>::new();
    for edge_id in 0..edge_index.edge_count() {
        let edge = edge_index.edge(edge_id).expect("edge identity is in range");
        let is_status_segment = match axis {
            SweepAxis::Horizontal => !edge.is_horizontal(),
            SweepAxis::Vertical => edge.is_horizontal(),
        };
        if !is_status_segment {
            continue;
        }
        let (start, end) = match axis {
            SweepAxis::Horizontal => (edge.bottom(), edge.top()),
            SweepAxis::Vertical => (edge.left(), edge.right()),
        };
        events.entry(start).or_default().insertions.push(edge_id);
        events.entry(end).or_default().removals.push(edge_id);
    }
    let reflex_points = boundary
        .reflex_vertices
        .iter()
        .map(|vertex| vertex.point)
        .collect::<BTreeSet<_>>();
    for &point in &reflex_points {
        let vertex_id = boundary_index
            .vertex_id(point)
            .ok_or(PolygonSgError::EndpointNotOnBoundary { point })?;
        let coordinate = match axis {
            SweepAxis::Horizontal => point.y,
            SweepAxis::Vertical => point.x,
        };
        events
            .entry(coordinate)
            .or_default()
            .queries
            .push((point, vertex_id));
    }

    let mut status = BTreeSet::<(i64, usize)>::new();
    let mut outputs = BTreeSet::<(i64, i64, i64)>::new();
    let mut certificate = SweepCertificate::default();
    for (coordinate, mut bucket) in events {
        bucket.insertions.sort_unstable();
        bucket.removals.sort_unstable();
        bucket.queries.sort_unstable_by_key(|&(point, vertex_id)| {
            (
                match axis {
                    SweepAxis::Horizontal => point.x,
                    SweepAxis::Vertical => point.y,
                },
                vertex_id,
            )
        });
        record_sweep_events(
            metrics,
            axis,
            bucket.insertions.len() + bucket.queries.len() + bucket.removals.len(),
        );
        for edge_id in &bucket.insertions {
            let edge = edge_index
                .edge(*edge_id)
                .expect("edge identity is in range");
            let transverse = match axis {
                SweepAxis::Horizontal => edge.first.x,
                SweepAxis::Vertical => edge.first.y,
            };
            status.insert((transverse, *edge_id));
            metrics.sweep_status_insertions += 1;
            metrics.sweep_auxiliary_tree_operations += 1;
        }
        for &(source_point, source) in &bucket.queries {
            metrics.sweep_status_queries += 1;
            metrics.sweep_auxiliary_tree_operations += 1;
            let transverse = match axis {
                SweepAxis::Horizontal => source_point.x,
                SweepAxis::Vertical => source_point.y,
            };
            let increasing = sweep_interior_direction(boundary, source, axis)?;
            let blocker = if increasing {
                status
                    .range((Excluded((transverse, usize::MAX)), Unbounded))
                    .next()
                    .copied()
            } else {
                status
                    .range((Unbounded, Excluded((transverse, usize::MIN))))
                    .next_back()
                    .copied()
            };
            let Some((_, blocker_edge_id)) = blocker else {
                continue;
            };
            let blocker_edge = edge_index
                .edge(blocker_edge_id)
                .expect("status edge identity is in range");
            let target_point = match axis {
                SweepAxis::Horizontal => Point::new(blocker_edge.first.x, coordinate),
                SweepAxis::Vertical => Point::new(coordinate, blocker_edge.first.y),
            };
            let Some(target) = boundary_index.vertex_id(target_point) else {
                continue;
            };
            if !reflex_points.contains(&target_point) || source_point >= target_point {
                continue;
            }
            let key = match axis {
                SweepAxis::Horizontal => (coordinate, source_point.x, target_point.x),
                SweepAxis::Vertical => (coordinate, source_point.y, target_point.y),
            };
            if !outputs.insert(key) {
                metrics.sweep_duplicate_output_count += 1;
                continue;
            }
            match axis {
                SweepAxis::Horizontal => metrics.sweep_output_horizontal_chords += 1,
                SweepAxis::Vertical => metrics.sweep_output_vertical_chords += 1,
            }
            certificate.output_records.push(SweepOutputRecord {
                axis,
                source,
                target,
                source_point,
                target_point,
                blocker_edge_id,
            });
        }
        for edge_id in &bucket.removals {
            let edge = edge_index
                .edge(*edge_id)
                .expect("edge identity is in range");
            let transverse = match axis {
                SweepAxis::Horizontal => edge.first.x,
                SweepAxis::Vertical => edge.first.y,
            };
            status.remove(&(transverse, *edge_id));
            metrics.sweep_status_deletions += 1;
            metrics.sweep_auxiliary_tree_operations += 1;
        }
        if certificate.event_summaries.len() < SWEEP_EVENT_TRACE_LIMIT {
            certificate.event_summaries.push(SweepEventSummary {
                axis,
                coordinate,
                inserted_segment_count: bucket.insertions.len(),
                query_count: bucket.queries.len(),
                removed_segment_count: bucket.removals.len(),
                insert_query_remove_order: true,
            });
        } else {
            certificate.event_trace_truncated = true;
        }
    }
    Ok((outputs, certificate))
}

fn record_sweep_events(
    metrics: &mut PolygonChordEnumerationMetrics,
    axis: SweepAxis,
    count: usize,
) {
    match axis {
        SweepAxis::Horizontal => metrics.sweep_horizontal_event_count += count,
        SweepAxis::Vertical => metrics.sweep_vertical_event_count += count,
    }
}

fn sweep_interior_direction(
    boundary: &Boundary,
    vertex_id: BoundaryVertexId,
    axis: SweepAxis,
) -> Result<bool, PolygonSgError> {
    let (previous, current, next) = incident_vertices(boundary, vertex_id)?;
    let neighbor = match axis {
        SweepAxis::Horizontal if previous.y == current.y => previous,
        SweepAxis::Horizontal if next.y == current.y => next,
        SweepAxis::Vertical if previous.x == current.x => previous,
        SweepAxis::Vertical if next.x == current.x => next,
        _ => return Err(PolygonSgError::InvalidBoundaryVertexId(vertex_id)),
    };
    Ok(match axis {
        SweepAxis::Horizontal => neighbor.x < current.x,
        SweepAxis::Vertical => neighbor.y < current.y,
    })
}

fn horizontal_satisfies_definition_7_indexed(
    prepared: &PreparedPolygonContext,
    left: i64,
    right: i64,
    y: i64,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<bool, PolygonSgError> {
    if prepared
        .edge_index()
        .horizontal_collinear_overlap(y, left, right)
    {
        return Ok(false);
    }
    metrics.polygon_segment_reporting_queries += 1;
    let crossing_ids =
        prepared
            .edge_index()
            .report_vertical_crossings(2 * i128::from(y), left, right);
    metrics.polygon_reported_boundary_intersections += crossing_ids.len();
    let mut breaks = BTreeSet::from([2 * i128::from(left), 2 * i128::from(right)]);
    for edge_id in crossing_ids {
        let edge = prepared
            .edge_index()
            .edge(edge_id)
            .expect("reported edge identity is indexed");
        let point = Point::new(edge.first.x, y);
        let Some(vertex_id) = prepared.boundary_index().vertex_id(point) else {
            return Ok(false);
        };
        if orthogonal_incident_edge_count(prepared.boundary(), vertex_id, true)? != 1 {
            return Ok(false);
        }
        breaks.insert(2 * i128::from(point.x));
    }
    indexed_horizontal_subintervals_are_interior(prepared, &breaks, y, metrics)
}

fn vertical_satisfies_definition_7_indexed(
    prepared: &PreparedPolygonContext,
    x: i64,
    bottom: i64,
    top: i64,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<bool, PolygonSgError> {
    if prepared
        .edge_index()
        .vertical_collinear_overlap(x, bottom, top)
    {
        return Ok(false);
    }
    metrics.polygon_segment_reporting_queries += 1;
    let crossing_ids =
        prepared
            .edge_index()
            .report_horizontal_crossings(2 * i128::from(x), bottom, top);
    metrics.polygon_reported_boundary_intersections += crossing_ids.len();
    let mut breaks = BTreeSet::from([2 * i128::from(bottom), 2 * i128::from(top)]);
    for edge_id in crossing_ids {
        let edge = prepared
            .edge_index()
            .edge(edge_id)
            .expect("reported edge identity is indexed");
        let point = Point::new(x, edge.first.y);
        let Some(vertex_id) = prepared.boundary_index().vertex_id(point) else {
            return Ok(false);
        };
        if orthogonal_incident_edge_count(prepared.boundary(), vertex_id, false)? != 1 {
            return Ok(false);
        }
        breaks.insert(2 * i128::from(point.y));
    }
    indexed_vertical_subintervals_are_interior(prepared, &breaks, x, metrics)
}

fn indexed_horizontal_subintervals_are_interior(
    prepared: &PreparedPolygonContext,
    breaks: &BTreeSet<i128>,
    y: i64,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<bool, PolygonSgError> {
    let coordinates = breaks.iter().copied().collect::<Vec<_>>();
    for pair in coordinates.windows(2) {
        let doubled_x = pair[0]
            .checked_add(pair[1])
            .and_then(|sum| sum.checked_div(2))
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        metrics.polygon_point_location_queries += 1;
        if !prepared
            .edge_index()
            .contains_doubled_point_strict(DoubledPoint::new(doubled_x, 2 * i128::from(y)))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn indexed_vertical_subintervals_are_interior(
    prepared: &PreparedPolygonContext,
    breaks: &BTreeSet<i128>,
    x: i64,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<bool, PolygonSgError> {
    let coordinates = breaks.iter().copied().collect::<Vec<_>>();
    for pair in coordinates.windows(2) {
        let doubled_y = pair[0]
            .checked_add(pair[1])
            .and_then(|sum| sum.checked_div(2))
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        metrics.polygon_point_location_queries += 1;
        if !prepared
            .edge_index()
            .contains_doubled_point_strict(DoubledPoint::new(2 * i128::from(x), doubled_y))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Classifies a boundary-native polygon for the clean hole-free path-tree
/// representation without consulting grid occupancy.
#[must_use]
pub fn classify_clean_polygon(
    polygon: &RectilinearPolygon,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    endpoint_index: &EffectiveChordEndpointIndex,
) -> CleanHoleFreeCertificate {
    let mut rejection_reasons = Vec::new();
    let outer_loop_count = boundary.outer_loop_count();
    let hole_count = boundary.hole_count();
    if outer_loop_count != 1 {
        rejection_reasons.push(CleanRejectionReason::MultipleOuterLoops {
            count: outer_loop_count,
        });
    }
    if hole_count != 0 {
        rejection_reasons.push(CleanRejectionReason::HasHole { count: hole_count });
    }
    let mut endpoint_owners = BTreeMap::<BoundaryVertexId, Vec<ChordRef>>::new();
    let mut all_chords_proper = true;
    for (index, &chord) in horizontal_chords.iter().enumerate() {
        let proper = polygon.contains_open_horizontal_segment(
            chord.left(),
            chord.right(),
            2 * i128::from(chord.y()),
        );
        all_chords_proper &= proper;
        if !proper {
            rejection_reasons.push(CleanRejectionReason::NonProperHorizontalChord(chord.id()));
        }
        if let Some(endpoints) = endpoint_index.horizontal.get(index) {
            endpoint_owners
                .entry(endpoints.first)
                .or_default()
                .push(ChordRef::Horizontal(chord.id()));
            endpoint_owners
                .entry(endpoints.second)
                .or_default()
                .push(ChordRef::Horizontal(chord.id()));
        } else {
            all_chords_proper = false;
            rejection_reasons.push(CleanRejectionReason::EndpointNotOnBoundary);
        }
    }
    for (index, &chord) in vertical_chords.iter().enumerate() {
        let proper = polygon.contains_open_vertical_segment(
            2 * i128::from(chord.x()),
            chord.bottom(),
            chord.top(),
        );
        all_chords_proper &= proper;
        if !proper {
            rejection_reasons.push(CleanRejectionReason::NonProperVerticalChord(chord.id()));
        }
        if let Some(endpoints) = endpoint_index.vertical.get(index) {
            endpoint_owners
                .entry(endpoints.first)
                .or_default()
                .push(ChordRef::Vertical(chord.id()));
            endpoint_owners
                .entry(endpoints.second)
                .or_default()
                .push(ChordRef::Vertical(chord.id()));
        } else {
            all_chords_proper = false;
            rejection_reasons.push(CleanRejectionReason::EndpointNotOnBoundary);
        }
    }
    let distinct_boundary_endpoints = endpoint_owners.values().all(|owners| owners.len() <= 1);
    if !distinct_boundary_endpoints {
        for (endpoint, mut owners) in endpoint_owners {
            owners.sort_unstable();
            for first in 0..owners.len() {
                for second in first + 1..owners.len() {
                    rejection_reasons.push(CleanRejectionReason::SharedBoundaryEndpoint {
                        first: owners[first],
                        second: owners[second],
                        endpoint,
                    });
                }
            }
        }
    }
    CleanHoleFreeCertificate {
        eligible: rejection_reasons.is_empty(),
        outer_loop_count,
        hole_count,
        all_chords_proper,
        distinct_boundary_endpoints,
        rejection_reasons,
    }
}

fn endpoint_has_collinear_edge(
    boundary: &Boundary,
    boundary_index: &BoundaryIndex,
    point: Point,
    horizontal: bool,
) -> Result<bool, PolygonSgError> {
    let id = boundary_index
        .vertex_id(point)
        .ok_or(PolygonSgError::EndpointNotOnBoundary { point })?;
    let (previous, current, next) = incident_vertices(boundary, id)?;
    Ok(if horizontal {
        previous.y == current.y || next.y == current.y
    } else {
        previous.x == current.x || next.x == current.x
    })
}

fn horizontal_satisfies_definition_7(
    polygon: &RectilinearPolygon,
    boundary: &Boundary,
    boundary_index: &BoundaryIndex,
    left: i64,
    right: i64,
    y: i64,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<bool, PolygonSgError> {
    metrics.polygon_definition7_full_boundary_scans += 1;
    metrics.polygon_boundary_edge_visits += boundary.boundary_complexity();
    let mut breaks = BTreeSet::from([2 * i128::from(left), 2 * i128::from(right)]);
    for boundary_loop in &boundary.loops {
        for index in 0..boundary_loop.vertices.len() {
            let first = boundary_loop.vertices[index];
            let second = boundary_loop.vertices[(index + 1) % boundary_loop.vertices.len()];
            if first.y == second.y {
                if first.y == y
                    && left.max(first.x.min(second.x)) < right.min(first.x.max(second.x))
                {
                    return Ok(false);
                }
                continue;
            }
            let edge_bottom = first.y.min(second.y);
            let edge_top = first.y.max(second.y);
            if left < first.x && first.x < right && edge_bottom <= y && y <= edge_top {
                let point = Point::new(first.x, y);
                let Some(vertex_id) = boundary_index.vertex_id(point) else {
                    return Ok(false);
                };
                if orthogonal_incident_edge_count(boundary, vertex_id, true)? != 1 {
                    return Ok(false);
                }
                breaks.insert(2 * i128::from(first.x));
            }
        }
    }
    all_horizontal_subintervals_are_interior(polygon, &breaks, y)
}

fn vertical_satisfies_definition_7(
    polygon: &RectilinearPolygon,
    boundary: &Boundary,
    boundary_index: &BoundaryIndex,
    x: i64,
    bottom: i64,
    top: i64,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<bool, PolygonSgError> {
    metrics.polygon_definition7_full_boundary_scans += 1;
    metrics.polygon_boundary_edge_visits += boundary.boundary_complexity();
    let mut breaks = BTreeSet::from([2 * i128::from(bottom), 2 * i128::from(top)]);
    for boundary_loop in &boundary.loops {
        for index in 0..boundary_loop.vertices.len() {
            let first = boundary_loop.vertices[index];
            let second = boundary_loop.vertices[(index + 1) % boundary_loop.vertices.len()];
            if first.x == second.x {
                if first.x == x
                    && bottom.max(first.y.min(second.y)) < top.min(first.y.max(second.y))
                {
                    return Ok(false);
                }
                continue;
            }
            let edge_left = first.x.min(second.x);
            let edge_right = first.x.max(second.x);
            if bottom < first.y && first.y < top && edge_left <= x && x <= edge_right {
                let point = Point::new(x, first.y);
                let Some(vertex_id) = boundary_index.vertex_id(point) else {
                    return Ok(false);
                };
                if orthogonal_incident_edge_count(boundary, vertex_id, false)? != 1 {
                    return Ok(false);
                }
                breaks.insert(2 * i128::from(first.y));
            }
        }
    }
    all_vertical_subintervals_are_interior(polygon, &breaks, x)
}

fn all_horizontal_subintervals_are_interior(
    polygon: &RectilinearPolygon,
    breaks: &BTreeSet<i128>,
    y: i64,
) -> Result<bool, PolygonSgError> {
    let coordinates = breaks.iter().copied().collect::<Vec<_>>();
    for pair in coordinates.windows(2) {
        let doubled_x = pair[0]
            .checked_add(pair[1])
            .and_then(|sum| sum.checked_div(2))
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        if !polygon.contains_doubled_point_strict(DoubledPoint::new(doubled_x, 2 * i128::from(y))) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn all_vertical_subintervals_are_interior(
    polygon: &RectilinearPolygon,
    breaks: &BTreeSet<i128>,
    x: i64,
) -> Result<bool, PolygonSgError> {
    let coordinates = breaks.iter().copied().collect::<Vec<_>>();
    for pair in coordinates.windows(2) {
        let doubled_y = pair[0]
            .checked_add(pair[1])
            .and_then(|sum| sum.checked_div(2))
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        if !polygon.contains_doubled_point_strict(DoubledPoint::new(2 * i128::from(x), doubled_y)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn orthogonal_incident_edge_count(
    boundary: &Boundary,
    vertex_id: BoundaryVertexId,
    horizontal_chord: bool,
) -> Result<usize, PolygonSgError> {
    let (previous, current, next) = incident_vertices(boundary, vertex_id)?;
    Ok([previous, next]
        .into_iter()
        .filter(|neighbor| {
            if horizontal_chord {
                neighbor.x == current.x
            } else {
                neighbor.y == current.y
            }
        })
        .count())
}

fn incident_vertices(
    boundary: &Boundary,
    vertex_id: BoundaryVertexId,
) -> Result<(Point, Point, Point), PolygonSgError> {
    let boundary_loop = boundary
        .loops
        .get(vertex_id.loop_id.0)
        .ok_or(PolygonSgError::InvalidBoundaryVertexId(vertex_id))?;
    let len = boundary_loop.vertices.len();
    let current = boundary_loop
        .vertices
        .get(vertex_id.cyclic_index)
        .copied()
        .ok_or(PolygonSgError::InvalidBoundaryVertexId(vertex_id))?;
    Ok((
        boundary_loop.vertices[(vertex_id.cyclic_index + len - 1) % len],
        current,
        boundary_loop.vertices[(vertex_id.cyclic_index + 1) % len],
    ))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum PolygonDirection {
    East,
    North,
    West,
    South,
}

impl PolygonDirection {
    const fn is_horizontal(self) -> bool {
        matches!(self, Self::East | Self::West)
    }

    const fn order(self) -> u8 {
        match self {
            Self::East => 0,
            Self::North => 1,
            Self::West => 2,
            Self::South => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PolygonFrontierCandidate {
    y: i64,
    x: i64,
    direction_order: u8,
    generation: u64,
    direction: PolygonDirection,
}

struct IndexedPolygonCompletionState<'a> {
    prepared: &'a PreparedPolygonContext,
    cuts: polygon_cut_index::Index,
    coordinate_universe: BTreeSet<i64>,
    candidates: BTreeSet<Point>,
    generations: BTreeMap<Point, u64>,
    frontier: BinaryHeap<Reverse<PolygonFrontierCandidate>>,
}

impl<'a> IndexedPolygonCompletionState<'a> {
    fn new(
        prepared: &'a PreparedPolygonContext,
        selected_horizontal: &[HorizontalCutSegment],
        selected_vertical: &[VerticalCutSegment],
        cut_index_backend: polygon_cut_index::Backend,
        metrics: &mut PolygonCompletionMetrics,
    ) -> Result<Self, PolygonSgError> {
        let coordinate_universe =
            completion_coordinate_universe(prepared, selected_horizontal, selected_vertical);
        let mut state = Self {
            prepared,
            cuts: polygon_cut_index::Index::new(cut_index_backend, coordinate_universe.clone())?,
            coordinate_universe,
            candidates: BTreeSet::new(),
            generations: BTreeMap::new(),
            frontier: BinaryHeap::new(),
        };
        for point in prepared
            .polygon()
            .loops()
            .flat_map(|boundary_loop| boundary_loop.vertices.iter().copied())
        {
            state.insert_candidate(point, metrics)?;
        }
        for &segment in selected_horizontal {
            let (inserted, intersections) =
                state.cuts.insert_horizontal_with_intersections(segment)?;
            if !inserted {
                return Err(PolygonSgError::InvalidSimpleChord {
                    start: Point::new(segment.left, segment.y),
                });
            }
            state.insert_candidate(Point::new(segment.left, segment.y), metrics)?;
            state.insert_candidate(Point::new(segment.right, segment.y), metrics)?;
            for point in intersections {
                metrics.completion_intersections_reported += 1;
                state.insert_candidate(point, metrics)?;
            }
        }
        for &segment in selected_vertical {
            let (inserted, intersections) =
                state.cuts.insert_vertical_with_intersections(segment)?;
            if !inserted {
                return Err(PolygonSgError::InvalidSimpleChord {
                    start: Point::new(segment.x, segment.bottom),
                });
            }
            state.insert_candidate(Point::new(segment.x, segment.bottom), metrics)?;
            state.insert_candidate(Point::new(segment.x, segment.top), metrics)?;
            for point in intersections {
                metrics.completion_intersections_reported += 1;
                state.insert_candidate(point, metrics)?;
            }
        }
        Ok(state)
    }

    fn ensure_completion_point(&self, point: Point) -> Result<(), PolygonSgError> {
        for coordinate in [point.x, point.y] {
            if !self.coordinate_universe.contains(&coordinate) {
                return Err(PolygonSgError::CompletionCoordinateOutsideUniverse { coordinate });
            }
        }
        Ok(())
    }

    fn insert_candidate(
        &mut self,
        point: Point,
        metrics: &mut PolygonCompletionMetrics,
    ) -> Result<(), PolygonSgError> {
        self.ensure_completion_point(point)?;
        if self.candidates.insert(point) {
            self.generations.insert(point, 0);
            metrics.completion_candidate_insertions += 1;
        }
        Ok(())
    }

    fn local_quadrants(&self, point: Point) -> [bool; 4] {
        let x = 2 * i128::from(point.x);
        let y = 2 * i128::from(point.y);
        [
            self.prepared
                .edge_index()
                .contains_doubled_point_strict(DoubledPoint::new(x - 1, y - 1)),
            self.prepared
                .edge_index()
                .contains_doubled_point_strict(DoubledPoint::new(x + 1, y - 1)),
            self.prepared
                .edge_index()
                .contains_doubled_point_strict(DoubledPoint::new(x + 1, y + 1)),
            self.prepared
                .edge_index()
                .contains_doubled_point_strict(DoubledPoint::new(x - 1, y + 1)),
        ]
    }

    fn local_blocked_rays(&mut self, inside: [bool; 4], point: Point) -> [bool; 4] {
        [
            self.cuts.contains_horizontal_ray(point, true) || inside[1] != inside[2],
            self.cuts.contains_vertical_ray(point, true) || inside[2] != inside[3],
            self.cuts.contains_horizontal_ray(point, false) || inside[3] != inside[0],
            self.cuts.contains_vertical_ray(point, false) || inside[0] != inside[1],
        ]
    }

    fn candidate_valid(
        &mut self,
        point: Point,
        direction: PolygonDirection,
        metrics: &mut PolygonCompletionMetrics,
    ) -> bool {
        if direction.is_horizontal() {
            metrics.horizontal_candidate_queries += 1;
        } else {
            metrics.vertical_candidate_queries += 1;
        }
        let inside = self.local_quadrants(point);
        let blocked = self.local_blocked_rays(inside, point);
        if !blocked.iter().any(|&value| value) {
            return false;
        }
        let (roots, sizes) = polygon_local_angle_components(inside, blocked);
        let (ray, first, second) = match direction {
            PolygonDirection::East => (0, 1, 2),
            PolygonDirection::North => (1, 2, 3),
            PolygonDirection::West => (2, 3, 0),
            PolygonDirection::South => (3, 0, 1),
        };
        inside[first]
            && inside[second]
            && !blocked[ray]
            && roots[first] == roots[second]
            && sizes[roots[first]] >= 3
    }

    fn enqueue_point(
        &mut self,
        point: Point,
        horizontal: bool,
        metrics: &mut PolygonCompletionMetrics,
    ) {
        let generation = self.generations.get(&point).copied().unwrap_or(0);
        for direction in [
            PolygonDirection::East,
            PolygonDirection::North,
            PolygonDirection::West,
            PolygonDirection::South,
        ] {
            if direction.is_horizontal() == horizontal
                && self.candidate_valid(point, direction, metrics)
            {
                self.frontier.push(Reverse(PolygonFrontierCandidate {
                    y: point.y,
                    x: point.x,
                    direction_order: direction.order(),
                    generation,
                    direction,
                }));
            }
        }
    }

    fn initialize_frontier(&mut self, horizontal: bool, metrics: &mut PolygonCompletionMetrics) {
        self.frontier.clear();
        let mut points = self.candidates.iter().copied().collect::<Vec<_>>();
        points.sort_unstable_by_key(|point| (point.y, point.x));
        for point in points {
            self.enqueue_point(point, horizontal, metrics);
        }
    }

    fn refresh_point(
        &mut self,
        point: Point,
        horizontal: bool,
        metrics: &mut PolygonCompletionMetrics,
    ) -> Result<(), PolygonSgError> {
        self.insert_candidate(point, metrics)?;
        *self.generations.entry(point).or_default() = self
            .generations
            .get(&point)
            .copied()
            .unwrap_or_default()
            .wrapping_add(1);
        self.enqueue_point(point, horizontal, metrics);
        Ok(())
    }

    fn pop_candidate(
        &mut self,
        metrics: &mut PolygonCompletionMetrics,
    ) -> Option<PolygonFrontierCandidate> {
        while let Some(Reverse(candidate)) = self.frontier.pop() {
            metrics.completion_candidate_revalidations += 1;
            let point = Point::new(candidate.x, candidate.y);
            let generation = self.generations.get(&point).copied().unwrap_or_default();
            if generation != candidate.generation
                || !self.candidate_valid(point, candidate.direction, metrics)
            {
                metrics.completion_stale_candidates += 1;
                continue;
            }
            return Some(candidate);
        }
        None
    }

    fn ray_stop(
        &mut self,
        point: Point,
        direction: PolygonDirection,
        metrics: &mut PolygonCompletionMetrics,
    ) -> Result<Option<Point>, PolygonSgError> {
        metrics.completion_boundary_ray_queries += 1;
        metrics.completion_cut_ray_queries += 1;
        let boundary = self.prepared.edge_index().nearest_boundary_blocker(
            point,
            match direction {
                PolygonDirection::East => rect_core::OrthogonalDirection::East,
                PolygonDirection::North => rect_core::OrthogonalDirection::North,
                PolygonDirection::West => rect_core::OrthogonalDirection::West,
                PolygonDirection::South => rect_core::OrthogonalDirection::South,
            },
        );
        let cut = self.cuts.nearest_blocker(point, direction);
        let stop = match direction {
            PolygonDirection::East => boundary.into_iter().chain(cut).min_by_key(|stop| stop.x),
            PolygonDirection::West => boundary.into_iter().chain(cut).max_by_key(|stop| stop.x),
            PolygonDirection::North => boundary.into_iter().chain(cut).min_by_key(|stop| stop.y),
            PolygonDirection::South => boundary.into_iter().chain(cut).max_by_key(|stop| stop.y),
        };
        if let Some(stop) = stop {
            self.ensure_completion_point(stop)?;
        }
        Ok(stop)
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_simple_chord(
        &mut self,
        point: Point,
        stop: Point,
        direction: PolygonDirection,
        horizontal_phase: bool,
        added_horizontal: &mut Vec<HorizontalCutSegment>,
        added_vertical: &mut Vec<VerticalCutSegment>,
        metrics: &mut PolygonCompletionMetrics,
    ) -> Result<(), PolygonSgError> {
        let mut affected = BTreeSet::from([point, stop]);
        match direction {
            PolygonDirection::East | PolygonDirection::West => {
                let segment =
                    HorizontalCutSegment::new(point.x.min(stop.x), point.x.max(stop.x), point.y)?;
                let (inserted, intersections) =
                    self.cuts.insert_horizontal_with_intersections(segment)?;
                if !inserted {
                    return Err(PolygonSgError::InvalidSimpleChord { start: point });
                }
                metrics.completion_intersections_reported += intersections.len();
                affected.extend(intersections);
                added_horizontal.push(segment);
                metrics.horizontal_simple_chord_count += 1;
            }
            PolygonDirection::North | PolygonDirection::South => {
                let segment =
                    VerticalCutSegment::new(point.x, point.y.min(stop.y), point.y.max(stop.y))?;
                let (inserted, intersections) =
                    self.cuts.insert_vertical_with_intersections(segment)?;
                if !inserted {
                    return Err(PolygonSgError::InvalidSimpleChord { start: point });
                }
                metrics.completion_intersections_reported += intersections.len();
                affected.extend(intersections);
                added_vertical.push(segment);
                metrics.vertical_simple_chord_count += 1;
            }
        }
        for affected_point in affected {
            self.refresh_point(affected_point, horizontal_phase, metrics)?;
        }
        Ok(())
    }

    fn complete_axis(
        &mut self,
        horizontal: bool,
        added_horizontal: &mut Vec<HorizontalCutSegment>,
        added_vertical: &mut Vec<VerticalCutSegment>,
        metrics: &mut PolygonCompletionMetrics,
    ) -> Result<(), PolygonSgError> {
        self.initialize_frontier(horizontal, metrics);
        let coordinate_bound = self
            .prepared
            .polygon()
            .boundary_complexity()
            .checked_add(self.cuts.horizontal_segments().len().saturating_mul(2))
            .and_then(|value| {
                value.checked_add(self.cuts.vertical_segments().len().saturating_mul(2))
            })
            .and_then(|value| value.checked_mul(value))
            .and_then(|value| value.checked_mul(4))
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        for _ in 0..=coordinate_bound {
            let Some(candidate) = self.pop_candidate(metrics) else {
                return Ok(());
            };
            let point = Point::new(candidate.x, candidate.y);
            let stop = self
                .ray_stop(point, candidate.direction, metrics)?
                .ok_or(PolygonSgError::UnboundedSimpleChord { start: point })?;
            self.insert_simple_chord(
                point,
                stop,
                candidate.direction,
                horizontal,
                added_horizontal,
                added_vertical,
                metrics,
            )?;
        }
        Err(PolygonSgError::CompletionDidNotTerminate)
    }
}

impl IndexedPolygonCompletion {
    /// Convenience API that prepares one polygon context internally.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid selection or completion geometry.
    pub fn complete(
        &self,
        polygon: &RectilinearPolygon,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            rect_core::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            rect_core::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        self.complete_prepared(
            &prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
        )
    }

    /// Completes selected chords using an incremental candidate frontier.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid selection or completion geometry.
    pub fn complete_prepared(
        &self,
        prepared: &PreparedPolygonContext,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        self.complete_prepared_with_backends(
            prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
            polygon_cut_index::Backend::Experiment,
            PolygonRecoveryBackend::SparseSubdivision,
            PolygonDissectionValidatorBackend::SparseSlab,
        )
    }

    /// Completes selected chords with an explicit mutable cut-index backend.
    ///
    /// The line-map backend is retained only as a completion differential
    /// oracle.  The dynamic backend uses the statically closed coordinate
    /// universe documented in `POLYGON_COMPLETION_COORDINATE_CLOSURE.md`.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid selection, completion geometry,
    /// or a violation of the coordinate-closure contract.
    pub fn complete_prepared_with_cut_index(
        &self,
        prepared: &PreparedPolygonContext,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
        cut_index_backend: polygon_cut_index::Backend,
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        self.complete_prepared_with_backends(
            prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
            cut_index_backend,
            PolygonRecoveryBackend::SparseSubdivision,
            PolygonDissectionValidatorBackend::SparseSlab,
        )
    }

    /// Completes selected chords with explicit index, recovery, and validation
    /// backends. Dense variants remain available for differential auditing.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for selection, coordinate-closure,
    /// completion, recovery, or validation failure.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn complete_prepared_with_backends(
        &self,
        prepared: &PreparedPolygonContext,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
        cut_index_backend: polygon_cut_index::Backend,
        recovery_backend: PolygonRecoveryBackend,
        validator_backend: PolygonDissectionValidatorBackend,
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        self.complete_prepared_with_geometry_backends(
            prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
            cut_index_backend,
            recovery_backend,
            validator_backend,
            polygon_sparse::subdivision::Backend::Experiment,
            SparseValidatorBackend::EventSegmentTree,
        )
    }

    /// Completes selected chords with explicit sparse subdivision and sparse
    /// validator implementations in addition to the preserved high-level
    /// backend selectors.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for selection, coordinate-closure,
    /// completion, recovery, or validation failure.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn complete_prepared_with_geometry_backends(
        &self,
        prepared: &PreparedPolygonContext,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
        cut_index_backend: polygon_cut_index::Backend,
        recovery_backend: PolygonRecoveryBackend,
        validator_backend: PolygonDissectionValidatorBackend,
        subdivision_builder_backend: polygon_sparse::subdivision::Backend,
        sparse_validator_backend: SparseValidatorBackend,
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        let started = Instant::now();
        if horizontal_chords.len() != selected_horizontal.len()
            || vertical_chords.len() != selected_vertical.len()
        {
            return Err(PolygonSgError::SelectionLengthMismatch);
        }
        let selected_horizontal_cuts = normalize_horizontal_segments(
            horizontal_chords
                .iter()
                .zip(selected_horizontal)
                .filter_map(|(&chord, &selected)| {
                    selected.then_some(HorizontalCutSegment::from_chord(chord))
                })
                .collect(),
        );
        let selected_vertical_cuts = normalize_vertical_segments(
            vertical_chords
                .iter()
                .zip(selected_vertical)
                .filter_map(|(&chord, &selected)| {
                    selected.then_some(VerticalCutSegment::from_chord(chord))
                })
                .collect(),
        );
        let mut metrics = PolygonCompletionMetrics::default();
        let mut state = IndexedPolygonCompletionState::new(
            prepared,
            &selected_horizontal_cuts,
            &selected_vertical_cuts,
            cut_index_backend,
            &mut metrics,
        )?;
        let selected_at = Instant::now();
        metrics.selected_cut_materialization_microseconds =
            selected_at.duration_since(started).as_micros();
        let mut added_horizontal_cuts = Vec::new();
        let mut added_vertical_cuts = Vec::new();
        state.complete_axis(
            true,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;
        let horizontal_at = Instant::now();
        metrics.horizontal_completion_microseconds =
            horizontal_at.duration_since(selected_at).as_micros();
        state.complete_axis(
            false,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;
        let vertical_at = Instant::now();
        metrics.vertical_completion_microseconds =
            vertical_at.duration_since(horizontal_at).as_micros();

        added_horizontal_cuts = normalize_horizontal_segments(added_horizontal_cuts);
        added_vertical_cuts = normalize_vertical_segments(added_vertical_cuts);
        let horizontal_cuts = state
            .cuts
            .horizontal_segments()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let vertical_cuts = state
            .cuts
            .vertical_segments()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let (x_count, y_count) =
            completion_coordinate_axis_counts(prepared, &horizontal_cuts, &vertical_cuts);
        metrics.coordinate_compression_x_count = x_count;
        metrics.coordinate_compression_y_count = y_count;
        recovery_backend
            .name()
            .clone_into(&mut metrics.recovery_policy);
        let final_segment_count = prepared
            .polygon()
            .boundary_complexity()
            .saturating_add(horizontal_cuts.len())
            .saturating_add(vertical_cuts.len());
        metrics.dense_recovery_retained_byte_estimate = x_count
            .saturating_sub(1)
            .saturating_mul(y_count.saturating_sub(1))
            .saturating_mul(4);
        metrics.sparse_recovery_retained_upper_estimate = final_segment_count.saturating_mul(
            2 * std::mem::size_of::<crate::polygon_sparse::SubdivisionVertex>()
                + 4 * std::mem::size_of::<crate::polygon_sparse::SubdivisionHalfEdge>()
                + 2 * std::mem::size_of::<crate::polygon_sparse::SubdivisionAtomicSegment>(),
        );
        let selected_recovery_backend = match recovery_backend {
            PolygonRecoveryBackend::Auto => {
                if metrics.dense_recovery_retained_byte_estimate
                    <= metrics.sparse_recovery_retained_upper_estimate
                {
                    PolygonRecoveryBackend::DenseCoordinateArrangement
                } else {
                    PolygonRecoveryBackend::SparseSubdivision
                }
            }
            backend => backend,
        };
        selected_recovery_backend
            .name()
            .clone_into(&mut metrics.selected_recovery_backend);
        let mut dense_arrangement = None;
        let rectangles = match selected_recovery_backend {
            PolygonRecoveryBackend::DenseCoordinateArrangement => {
                let mut arrangement = polygon_arrangement::Arrangement::new(
                    prepared,
                    &horizontal_cuts,
                    &vertical_cuts,
                )?;
                let rectangles = arrangement.recover_rectangles()?;
                metrics.coordinate_compression_x_count = arrangement.metrics().arrangement_x_count;
                metrics.coordinate_compression_y_count = arrangement.metrics().arrangement_y_count;
                metrics.atomic_cell_count = arrangement.metrics().arrangement_atomic_cells;
                metrics.rectangle_recovery_visits =
                    arrangement.metrics().arrangement_rectangle_recovery_visits;
                metrics.arrangement_point_location_queries =
                    arrangement.metrics().arrangement_point_location_queries;
                metrics.arrangement_boundary_edge_visits =
                    arrangement.metrics().arrangement_boundary_edge_visits;
                metrics.arrangement_span_writes = arrangement.metrics().arrangement_span_writes;
                metrics.arrangement_owned_bytes = arrangement.owned_bytes_estimate();
                dense_arrangement = Some(arrangement);
                rectangles
            }
            PolygonRecoveryBackend::SparseSubdivision => {
                let subdivision = SparseOrthogonalSubdivision::new_with_backend(
                    prepared,
                    &horizontal_cuts,
                    &vertical_cuts,
                    subdivision_builder_backend,
                )?;
                metrics.sparse_subdivision_vertices = subdivision.metrics.vertex_count;
                metrics.sparse_subdivision_half_edges = subdivision.metrics.half_edge_count;
                metrics.sparse_subdivision_faces = subdivision.metrics.face_count;
                metrics.sparse_subdivision_junctions = subdivision.metrics.junction_count;
                metrics.sparse_subdivision_owned_bytes = subdivision.metrics.owned_bytes;
                metrics.sparse_subdivision = subdivision.metrics.clone();
                subdivision.recover_rectangles(prepared.polygon())?
            }
            PolygonRecoveryBackend::Auto => unreachable!("auto recovery is resolved above"),
        };
        let recovered_at = Instant::now();
        metrics.rectangle_recovery_microseconds =
            recovered_at.duration_since(vertical_at).as_micros();
        match validator_backend {
            PolygonDissectionValidatorBackend::DenseArrangement => {
                let arrangement = match dense_arrangement {
                    Some(arrangement) => arrangement,
                    None => polygon_arrangement::Arrangement::new(
                        prepared,
                        &horizontal_cuts,
                        &vertical_cuts,
                    )?,
                };
                polygon_arrangement::experiment::Validator.validate(
                    &arrangement,
                    prepared.polygon(),
                    &rectangles,
                )?;
                metrics.coordinate_compression_x_count = arrangement.metrics().arrangement_x_count;
                metrics.coordinate_compression_y_count = arrangement.metrics().arrangement_y_count;
                metrics.atomic_cell_count = arrangement.metrics().arrangement_atomic_cells;
                metrics.arrangement_owned_bytes = arrangement.owned_bytes_estimate();
            }
            PolygonDissectionValidatorBackend::SparseSlab => {
                let slab = SparseSlabValidator.validate_with_backend(
                    prepared.polygon(),
                    &rectangles,
                    sparse_validator_backend,
                )?;
                metrics.sparse_validator_slab_count = slab.slab_count;
                metrics.sparse_validator = slab;
            }
        }
        metrics.final_validation_microseconds = recovered_at.elapsed().as_micros();
        metrics.cut_index = state.cuts.metrics();
        Ok(PolygonCompletionResult {
            rectangles,
            selected_horizontal_cuts,
            selected_vertical_cuts,
            added_horizontal_cuts,
            added_vertical_cuts,
            metrics,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "indexed-frontier"
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_polygon_axis(
    polygon: &RectilinearPolygon,
    extra_candidate_points: &BTreeSet<Point>,
    horizontal_cuts: &mut BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &mut BTreeSet<VerticalCutSegment>,
    horizontal: bool,
    added_horizontal: &mut Vec<HorizontalCutSegment>,
    added_vertical: &mut Vec<VerticalCutSegment>,
    metrics: &mut PolygonCompletionMetrics,
) -> Result<(), PolygonSgError> {
    let coordinate_bound = polygon
        .boundary_complexity()
        .checked_add(horizontal_cuts.len().saturating_mul(2))
        .and_then(|value| value.checked_add(vertical_cuts.len().saturating_mul(2)))
        .and_then(|value| value.checked_mul(value))
        .and_then(|value| value.checked_mul(4))
        .ok_or(PolygonSgError::CoordinateOverflow)?;
    for _ in 0..=coordinate_bound {
        let Some((point, direction)) = find_polygon_concave_ray(
            polygon,
            extra_candidate_points,
            horizontal_cuts,
            vertical_cuts,
            horizontal,
            metrics,
        ) else {
            return Ok(());
        };
        let stop = find_polygon_ray_stop(
            polygon,
            horizontal_cuts,
            vertical_cuts,
            point,
            direction,
            metrics,
        )
        .ok_or(PolygonSgError::UnboundedSimpleChord { start: point })?;
        match direction {
            PolygonDirection::East | PolygonDirection::West => {
                let segment =
                    HorizontalCutSegment::new(point.x.min(stop.x), point.x.max(stop.x), point.y)?;
                if !horizontal_cuts.insert(segment) {
                    return Err(PolygonSgError::InvalidSimpleChord { start: point });
                }
                added_horizontal.push(segment);
                metrics.horizontal_simple_chord_count += 1;
            }
            PolygonDirection::North | PolygonDirection::South => {
                let segment =
                    VerticalCutSegment::new(point.x, point.y.min(stop.y), point.y.max(stop.y))?;
                if !vertical_cuts.insert(segment) {
                    return Err(PolygonSgError::InvalidSimpleChord { start: point });
                }
                added_vertical.push(segment);
                metrics.vertical_simple_chord_count += 1;
            }
        }
    }
    Err(PolygonSgError::CompletionDidNotTerminate)
}

fn find_polygon_concave_ray(
    polygon: &RectilinearPolygon,
    extra_candidate_points: &BTreeSet<Point>,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
    horizontal: bool,
    metrics: &mut PolygonCompletionMetrics,
) -> Option<(Point, PolygonDirection)> {
    metrics.completion_global_candidate_rebuilds += 1;
    let candidates = polygon_candidate_points(
        polygon,
        extra_candidate_points,
        horizontal_cuts,
        vertical_cuts,
        metrics,
    );
    for point in candidates {
        let inside = polygon_local_quadrants(polygon, point);
        let blocked = polygon_local_blocked_rays(horizontal_cuts, vertical_cuts, inside, point);
        let isolated = !blocked.iter().any(|&value| value);
        if horizontal && !isolated && !blocked[0] && !blocked[2] {
            continue;
        }
        let (roots, sizes) = polygon_local_angle_components(inside, blocked);
        for (direction, ray, first, second) in [
            (PolygonDirection::East, 0, 1, 2),
            (PolygonDirection::North, 1, 2, 3),
            (PolygonDirection::West, 2, 3, 0),
            (PolygonDirection::South, 3, 0, 1),
        ] {
            if direction.is_horizontal() != horizontal {
                continue;
            }
            if horizontal {
                metrics.horizontal_candidate_queries += 1;
            } else {
                metrics.vertical_candidate_queries += 1;
            }
            if inside[first]
                && inside[second]
                && !blocked[ray]
                && roots[first] == roots[second]
                && sizes[roots[first]] >= 3
            {
                return Some((point, direction));
            }
        }
    }
    None
}

fn polygon_candidate_points(
    polygon: &RectilinearPolygon,
    extra_candidate_points: &BTreeSet<Point>,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
    metrics: &mut PolygonCompletionMetrics,
) -> Vec<Point> {
    let mut points = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().copied())
        .collect::<BTreeSet<_>>();
    points.extend(extra_candidate_points);
    for segment in horizontal_cuts {
        points.insert(Point::new(segment.left, segment.y));
        points.insert(Point::new(segment.right, segment.y));
    }
    for segment in vertical_cuts {
        points.insert(Point::new(segment.x, segment.bottom));
        points.insert(Point::new(segment.x, segment.top));
    }
    for horizontal in horizontal_cuts {
        for vertical in vertical_cuts {
            metrics.completion_cut_pair_tests += 1;
            if horizontal.left <= vertical.x
                && vertical.x <= horizontal.right
                && vertical.bottom <= horizontal.y
                && horizontal.y <= vertical.top
            {
                points.insert(Point::new(vertical.x, horizontal.y));
            }
        }
    }
    let mut points = points.into_iter().collect::<Vec<_>>();
    points.sort_unstable_by_key(|point| (point.y, point.x));
    points
}

fn polygon_local_quadrants(polygon: &RectilinearPolygon, point: Point) -> [bool; 4] {
    let x = 2 * i128::from(point.x);
    let y = 2 * i128::from(point.y);
    [
        polygon.contains_doubled_point_strict(DoubledPoint::new(x - 1, y - 1)),
        polygon.contains_doubled_point_strict(DoubledPoint::new(x + 1, y - 1)),
        polygon.contains_doubled_point_strict(DoubledPoint::new(x + 1, y + 1)),
        polygon.contains_doubled_point_strict(DoubledPoint::new(x - 1, y + 1)),
    ]
}

fn polygon_local_blocked_rays(
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
    inside: [bool; 4],
    point: Point,
) -> [bool; 4] {
    let east_cut = horizontal_cuts
        .iter()
        .any(|cut| cut.y == point.y && cut.left <= point.x && point.x < cut.right);
    let north_cut = vertical_cuts
        .iter()
        .any(|cut| cut.x == point.x && cut.bottom <= point.y && point.y < cut.top);
    let west_cut = horizontal_cuts
        .iter()
        .any(|cut| cut.y == point.y && cut.left < point.x && point.x <= cut.right);
    let south_cut = vertical_cuts
        .iter()
        .any(|cut| cut.x == point.x && cut.bottom < point.y && point.y <= cut.top);
    [
        east_cut || inside[1] != inside[2],
        north_cut || inside[2] != inside[3],
        west_cut || inside[3] != inside[0],
        south_cut || inside[0] != inside[1],
    ]
}

fn polygon_local_angle_components(
    inside: [bool; 4],
    blocked: [bool; 4],
) -> ([usize; 4], [usize; 4]) {
    let mut roots = [0, 1, 2, 3];
    for (ray, first, second) in [(0, 1, 2), (1, 2, 3), (2, 3, 0), (3, 0, 1)] {
        if inside[first] && inside[second] && !blocked[ray] {
            polygon_union_roots(&mut roots, first, second);
        }
    }
    for index in 0..4 {
        roots[index] = polygon_find_root(&roots, index);
    }
    let mut sizes = [0; 4];
    for index in 0..4 {
        if inside[index] {
            sizes[roots[index]] += 1;
        }
    }
    (roots, sizes)
}

fn polygon_find_root(roots: &[usize; 4], mut index: usize) -> usize {
    while roots[index] != index {
        index = roots[index];
    }
    index
}

fn polygon_union_roots(roots: &mut [usize; 4], first: usize, second: usize) {
    let first_root = polygon_find_root(roots, first);
    let second_root = polygon_find_root(roots, second);
    if first_root != second_root {
        roots[second_root] = first_root;
    }
}

fn find_polygon_ray_stop(
    polygon: &RectilinearPolygon,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
    point: Point,
    direction: PolygonDirection,
    metrics: &mut PolygonCompletionMetrics,
) -> Option<Point> {
    metrics.completion_boundary_ray_queries += 1;
    metrics.completion_full_boundary_scans += 1;
    metrics.completion_cut_ray_queries += 1;
    metrics.completion_full_cut_scans += 1;
    let mut coordinates = Vec::new();
    for boundary_loop in polygon.loops() {
        for (first, second) in boundary_loop.edges() {
            collect_boundary_stop(&mut coordinates, point, direction, first, second);
        }
    }
    match direction {
        PolygonDirection::East => {
            coordinates.extend(vertical_cuts.iter().filter_map(|cut| {
                (cut.x > point.x && cut.bottom <= point.y && point.y <= cut.top).then_some(cut.x)
            }));
            coordinates.extend(
                horizontal_cuts
                    .iter()
                    .filter_map(|cut| (cut.y == point.y && cut.left > point.x).then_some(cut.left)),
            );
            coordinates
                .into_iter()
                .min()
                .map(|x| Point::new(x, point.y))
        }
        PolygonDirection::West => {
            coordinates.extend(vertical_cuts.iter().filter_map(|cut| {
                (cut.x < point.x && cut.bottom <= point.y && point.y <= cut.top).then_some(cut.x)
            }));
            coordinates.extend(
                horizontal_cuts.iter().filter_map(|cut| {
                    (cut.y == point.y && cut.right < point.x).then_some(cut.right)
                }),
            );
            coordinates
                .into_iter()
                .max()
                .map(|x| Point::new(x, point.y))
        }
        PolygonDirection::North => {
            coordinates.extend(horizontal_cuts.iter().filter_map(|cut| {
                (cut.y > point.y && cut.left <= point.x && point.x <= cut.right).then_some(cut.y)
            }));
            coordinates.extend(vertical_cuts.iter().filter_map(|cut| {
                (cut.x == point.x && cut.bottom > point.y).then_some(cut.bottom)
            }));
            coordinates
                .into_iter()
                .min()
                .map(|y| Point::new(point.x, y))
        }
        PolygonDirection::South => {
            coordinates.extend(horizontal_cuts.iter().filter_map(|cut| {
                (cut.y < point.y && cut.left <= point.x && point.x <= cut.right).then_some(cut.y)
            }));
            coordinates.extend(
                vertical_cuts
                    .iter()
                    .filter_map(|cut| (cut.x == point.x && cut.top < point.y).then_some(cut.top)),
            );
            coordinates
                .into_iter()
                .max()
                .map(|y| Point::new(point.x, y))
        }
    }
}

fn collect_boundary_stop(
    coordinates: &mut Vec<i64>,
    point: Point,
    direction: PolygonDirection,
    first: Point,
    second: Point,
) {
    let left = first.x.min(second.x);
    let right = first.x.max(second.x);
    let bottom = first.y.min(second.y);
    let top = first.y.max(second.y);
    match direction {
        PolygonDirection::East if first.x == second.x => {
            if first.x > point.x && bottom <= point.y && point.y <= top {
                coordinates.push(first.x);
            }
        }
        PolygonDirection::East if first.y == second.y => {
            if first.y == point.y && left > point.x {
                coordinates.push(left);
            }
        }
        PolygonDirection::West if first.x == second.x => {
            if first.x < point.x && bottom <= point.y && point.y <= top {
                coordinates.push(first.x);
            }
        }
        PolygonDirection::West if first.y == second.y => {
            if first.y == point.y && right < point.x {
                coordinates.push(right);
            }
        }
        PolygonDirection::North if first.y == second.y => {
            if first.y > point.y && left <= point.x && point.x <= right {
                coordinates.push(first.y);
            }
        }
        PolygonDirection::North if first.x == second.x => {
            if first.x == point.x && bottom > point.y {
                coordinates.push(bottom);
            }
        }
        PolygonDirection::South if first.y == second.y => {
            if first.y < point.y && left <= point.x && point.x <= right {
                coordinates.push(first.y);
            }
        }
        PolygonDirection::South if first.x == second.x => {
            if first.x == point.x && top < point.y {
                coordinates.push(top);
            }
        }
        _ => {}
    }
}

struct PolygonRecovery {
    rectangles: Vec<CoordinateRect>,
    x_count: usize,
    y_count: usize,
    atomic_cell_count: usize,
    visits: usize,
}

fn recover_coordinate_rectangles(
    polygon: &RectilinearPolygon,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
) -> Result<PolygonRecovery, PolygonSgError> {
    let mut xs = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
        .collect::<BTreeSet<_>>();
    let mut ys = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
        .collect::<BTreeSet<_>>();
    for cut in horizontal_cuts {
        xs.extend([cut.left, cut.right]);
        ys.insert(cut.y);
    }
    for cut in vertical_cuts {
        xs.insert(cut.x);
        ys.extend([cut.bottom, cut.top]);
    }
    let xs = xs.into_iter().collect::<Vec<_>>();
    let ys = ys.into_iter().collect::<Vec<_>>();
    let width = xs.len().saturating_sub(1);
    let height = ys.len().saturating_sub(1);
    let atomic_cell_count = width
        .checked_mul(height)
        .ok_or(PolygonSgError::CoordinateOverflow)?;
    let occupied = (0..atomic_cell_count)
        .map(|index| {
            let x = index % width;
            let y = index / width;
            polygon.contains_doubled_point_strict(DoubledPoint::new(
                i128::from(xs[x]) + i128::from(xs[x + 1]),
                i128::from(ys[y]) + i128::from(ys[y + 1]),
            ))
        })
        .collect::<Vec<_>>();
    let mut region_ids = vec![usize::MAX; atomic_cell_count];
    let mut queue = VecDeque::new();
    let mut rectangles = Vec::new();
    let mut visits = 0;
    for seed in 0..atomic_cell_count {
        if !occupied[seed] || region_ids[seed] != usize::MAX {
            continue;
        }
        let region_id = rectangles.len();
        region_ids[seed] = region_id;
        queue.push_back(seed);
        let (mut left, mut right) = (seed % width, seed % width + 1);
        let (mut bottom, mut top) = (seed / width, seed / width + 1);
        while let Some(index) = queue.pop_front() {
            visits += 1;
            let x = index % width;
            let y = index / width;
            left = left.min(x);
            right = right.max(x + 1);
            bottom = bottom.min(y);
            top = top.max(y + 1);
            let mut visit = |neighbor: usize| {
                if occupied[neighbor] && region_ids[neighbor] == usize::MAX {
                    region_ids[neighbor] = region_id;
                    queue.push_back(neighbor);
                }
            };
            if x > 0 && !vertical_barrier_covers(vertical_cuts, xs[x], ys[y], ys[y + 1]) {
                visit(index - 1);
            }
            if x + 1 < width && !vertical_barrier_covers(vertical_cuts, xs[x + 1], ys[y], ys[y + 1])
            {
                visit(index + 1);
            }
            if y > 0 && !horizontal_barrier_covers(horizontal_cuts, ys[y], xs[x], xs[x + 1]) {
                visit(index - width);
            }
            if y + 1 < height
                && !horizontal_barrier_covers(horizontal_cuts, ys[y + 1], xs[x], xs[x + 1])
            {
                visit(index + width);
            }
        }
        if !(bottom..top).all(|y| (left..right).all(|x| region_ids[y * width + x] == region_id)) {
            return Err(PolygonSgError::NonRectangularCompletionRegion {
                point: Point::new(xs[seed % width], ys[seed / width]),
            });
        }
        rectangles.push(CoordinateRect::new(
            xs[left], ys[bottom], xs[right], ys[top],
        )?);
    }
    rectangles.sort_unstable();
    Ok(PolygonRecovery {
        rectangles,
        x_count: xs.len(),
        y_count: ys.len(),
        atomic_cell_count,
        visits,
    })
}

fn vertical_barrier_covers(
    cuts: &BTreeSet<VerticalCutSegment>,
    x: i64,
    bottom: i64,
    top: i64,
) -> bool {
    cuts.iter()
        .any(|cut| cut.x == x && cut.bottom <= bottom && top <= cut.top)
}

fn horizontal_barrier_covers(
    cuts: &BTreeSet<HorizontalCutSegment>,
    y: i64,
    left: i64,
    right: i64,
) -> bool {
    cuts.iter()
        .any(|cut| cut.y == y && cut.left <= left && right <= cut.right)
}

fn validate_formal_boundary_coverage(
    incidence: &FormalBoundaryIncidence,
    rectangles: &[CoordinateRect],
) -> Result<(), PolygonSgError> {
    let mut horizontal = BTreeMap::<i64, Vec<(i64, i64)>>::new();
    let mut vertical = BTreeMap::<i64, Vec<(i64, i64)>>::new();
    for rectangle in rectangles {
        horizontal
            .entry(rectangle.y0)
            .or_default()
            .push((rectangle.x0, rectangle.x1));
        horizontal
            .entry(rectangle.y1)
            .or_default()
            .push((rectangle.x0, rectangle.x1));
        vertical
            .entry(rectangle.x0)
            .or_default()
            .push((rectangle.y0, rectangle.y1));
        vertical
            .entry(rectangle.x1)
            .or_default()
            .push((rectangle.y0, rectangle.y1));
    }
    for intervals in horizontal.values_mut().chain(vertical.values_mut()) {
        *intervals = merge_intervals(std::mem::take(intervals));
    }
    for vertex in &incidence.vertices {
        let point = vertex.point;
        let covered = horizontal
            .get(&point.y)
            .is_some_and(|intervals| interval_union_covers(intervals, point.x, point.x))
            || vertical
                .get(&point.x)
                .is_some_and(|intervals| interval_union_covers(intervals, point.y, point.y));
        if !covered {
            return Err(PolygonSgError::FormalBoundaryPointNotCovered { point });
        }
    }
    for segment in &incidence.elementary_segments {
        let first = incidence.vertices[segment.start.0].point;
        let second = incidence.vertices[segment.end.0].point;
        let covered = if first.y == second.y {
            horizontal.get(&first.y).is_some_and(|intervals| {
                interval_union_covers(intervals, first.x.min(second.x), first.x.max(second.x))
            })
        } else {
            vertical.get(&first.x).is_some_and(|intervals| {
                interval_union_covers(intervals, first.y.min(second.y), first.y.max(second.y))
            })
        };
        if !covered {
            return Err(PolygonSgError::FormalBoundarySegmentNotCovered { first, second });
        }
    }
    Ok(())
}

fn merge_intervals(mut intervals: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    intervals.sort_unstable();
    let mut merged = Vec::<(i64, i64)>::new();
    for interval in intervals {
        if let Some(last) = merged.last_mut()
            && interval.0 <= last.1
        {
            last.1 = last.1.max(interval.1);
        } else {
            merged.push(interval);
        }
    }
    merged
}

fn interval_union_covers(intervals: &[(i64, i64)], low: i64, high: i64) -> bool {
    intervals
        .iter()
        .any(|&(start, end)| start <= low && high <= end)
}

/// Validates an exact coordinate-rectangle partition of an ordinary polygon.
///
/// # Errors
///
/// Returns [`PolygonValidationError`] for invalid rectangles, overlap,
/// uncovered polygon area, or coverage outside the polygon.
pub fn validate_polygon_dissection(
    polygon: &RectilinearPolygon,
    rectangles: &[CoordinateRect],
) -> Result<(), PolygonValidationError> {
    let mut xs = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
        .collect::<BTreeSet<_>>();
    let mut ys = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
        .collect::<BTreeSet<_>>();
    let mut rectangle_area = 0_i128;
    for (index, rectangle) in rectangles.iter().copied().enumerate() {
        if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
            return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
        }
        xs.extend([rectangle.x0, rectangle.x1]);
        ys.extend([rectangle.y0, rectangle.y1]);
        rectangle_area = rectangle_area
            .checked_add(rectangle.area())
            .ok_or(PolygonValidationError::AreaOverflow)?;
    }
    let polygon_area_twice = polygon
        .twice_signed_area()
        .map_err(PolygonValidationError::Polygon)?;
    if rectangle_area
        .checked_mul(2)
        .ok_or(PolygonValidationError::AreaOverflow)?
        != polygon_area_twice
    {
        return Err(PolygonValidationError::AreaMismatch {
            polygon_area_twice,
            rectangle_area_twice: rectangle_area * 2,
        });
    }
    let xs = xs.into_iter().collect::<Vec<_>>();
    let ys = ys.into_iter().collect::<Vec<_>>();
    for y in 0..ys.len().saturating_sub(1) {
        for x in 0..xs.len().saturating_sub(1) {
            let point = DoubledPoint::new(
                i128::from(xs[x]) + i128::from(xs[x + 1]),
                i128::from(ys[y]) + i128::from(ys[y + 1]),
            );
            let inside = polygon.contains_doubled_point_strict(point);
            let covering = rectangles
                .iter()
                .enumerate()
                .filter_map(|(index, rectangle)| {
                    rectangle
                        .contains_doubled_point_strict(point)
                        .then_some(index)
                })
                .collect::<Vec<_>>();
            if covering.len() > 1 {
                return Err(PolygonValidationError::Overlap {
                    first: covering[0],
                    second: covering[1],
                    point,
                });
            }
            if inside && covering.is_empty() {
                return Err(PolygonValidationError::UncoveredInterior { point });
            }
            if !inside && !covering.is_empty() {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: covering[0],
                    point,
                });
            }
        }
    }
    Ok(())
}

/// Validates the declared optimum count in addition to exact geometry.
///
/// # Errors
///
/// Returns [`PolygonValidationError::DeclaredCount`] on count mismatch, then
/// applies [`validate_polygon_dissection`].
pub fn validate_polygon_dissection_count(
    polygon: &RectilinearPolygon,
    declared: usize,
    rectangles: &[CoordinateRect],
) -> Result<(), PolygonValidationError> {
    if declared != rectangles.len() {
        return Err(PolygonValidationError::DeclaredCount {
            declared,
            actual: rectangles.len(),
        });
    }
    validate_polygon_dissection(polygon, rectangles)
}

#[derive(Clone, Debug, Eq, Error, PartialEq, Serialize, Deserialize)]
pub enum PolygonValidationError {
    #[error(transparent)]
    Polygon(PolygonError),
    #[error("declared rectangle count {declared} differs from actual count {actual}")]
    DeclaredCount { declared: usize, actual: usize },
    #[error("coordinate rectangle {rectangle} has non-positive area")]
    NonPositiveRectangle { rectangle: usize },
    #[error("coordinate rectangle {rectangle} covers outside point {point:?}")]
    OutsidePolygon {
        rectangle: usize,
        point: DoubledPoint,
    },
    #[error("coordinate rectangles {first} and {second} overlap near {point:?}")]
    Overlap {
        first: usize,
        second: usize,
        point: DoubledPoint,
    },
    #[error("polygon interior is uncovered near {point:?}")]
    UncoveredInterior { point: DoubledPoint },
    #[error(
        "rectangle area {rectangle_area_twice} does not equal polygon area {polygon_area_twice}"
    )]
    AreaMismatch {
        polygon_area_twice: i128,
        rectangle_area_twice: i128,
    },
    #[error("exact rectangle-area arithmetic overflowed i128")]
    AreaOverflow,
}

#[derive(Debug, Error)]
pub enum PolygonSgError {
    #[error(transparent)]
    Polygon(#[from] PolygonError),
    #[error(transparent)]
    BoundaryIndex(#[from] BoundaryIndexError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error("effective chord endpoint {point:?} is not a normalized boundary vertex")]
    EndpointNotOnBoundary { point: Point },
    #[error("invalid normalized boundary vertex identity {0:?}")]
    InvalidBoundaryVertexId(BoundaryVertexId),
    #[error("doubled-coordinate arithmetic overflowed")]
    CoordinateOverflow,
    #[error("completion coordinate {coordinate} is outside the proven static universe")]
    CompletionCoordinateOutsideUniverse { coordinate: i64 },
    #[error("effective-chord selection vectors have the wrong length")]
    SelectionLengthMismatch,
    #[error("simple chord from {start:?} is empty or duplicates an existing cut")]
    InvalidSimpleChord { start: Point },
    #[error("simple chord from {start:?} did not reach a boundary or existing cut")]
    UnboundedSimpleChord { start: Point },
    #[error("boundary-native completion did not terminate")]
    CompletionDidNotTerminate,
    #[error("completion region at {point:?} is not a coordinate rectangle")]
    NonRectangularCompletionRegion { point: Point },
    #[error("sparse polygon subdivision failed: {message}")]
    SparseSubdivision { message: String },
    #[error("ordinary-loop sweep audit failed: {message}")]
    SweepAuditFailed { message: String },
    #[error("formal completion failed: {message}")]
    Formal { message: String },
    #[error("dense and sparse formal rectangle recovery disagree")]
    FormalRecoveryMismatch,
    #[error("formal-boundary point {point:?} is absent from all rectangle boundaries")]
    FormalBoundaryPointNotCovered { point: Point },
    #[error(
        "formal elementary segment {first:?}--{second:?} is not covered by rectangle boundaries"
    )]
    FormalBoundarySegmentNotCovered { first: Point, second: Point },
    #[error(transparent)]
    Validation(#[from] PolygonValidationError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rect_core::{
        Boundary, BoundaryIndex, ColorGrid, CoordinateRect, FormalRectilinearPolygon, Ornament,
        OrthogonalLoop, Point, PreparedPolygonContext, RectilinearPolygon,
    };

    use crate::polygon_arrangement;
    use crate::{EffectiveChordEnumerator, GridInteriorRunEnumerator};

    use super::{
        CoordinateCompressedCompletion, GeneralPolygonPairwiseEnumerator, HorizontalCutSegment,
        IndexedPolygonCompletion, IndexedPolygonPairwiseEnumerator,
        SoltanGorpinevichSweepEnumerator, SweepAxis, VerticalCutSegment,
        endpoint_has_collinear_edge, horizontal_satisfies_definition_7, sweep_interior_direction,
    };

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    fn first_grid_derived_polygon_with_chords() -> RectilinearPolygon {
        for mask in 1_u16..1 << 9 {
            let grid =
                ColorGrid::new(3, 3, (0..9).map(|bit| mask & (1 << bit) != 0).collect()).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = Boundary::from_component(&component).unwrap();
                let Ok(polygon) = boundary.to_polygon() else {
                    continue;
                };
                let families = GeneralPolygonPairwiseEnumerator
                    .enumerate(&polygon)
                    .unwrap();
                if !families.horizontal.is_empty() || !families.vertical.is_empty() {
                    return polygon;
                }
            }
        }
        panic!("3x3 population must contain a polygon with effective chords");
    }

    #[test]
    fn formal_empty_ornament_matches_ordinary_local_measure_and_pairwise_chords() {
        let fixtures = [
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(6, 0),
                    Point::new(6, 2),
                    Point::new(2, 2),
                    Point::new(2, 6),
                    Point::new(0, 6),
                ]),
                vec![],
            )
            .unwrap(),
            RectilinearPolygon::new(rectangle(0, 0, 12, 10), vec![rectangle(4, 3, 8, 7)]).unwrap(),
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(12, 0),
                    Point::new(12, 10),
                    Point::new(9, 10),
                    Point::new(9, 4),
                    Point::new(7, 4),
                    Point::new(7, 10),
                    Point::new(5, 10),
                    Point::new(5, 4),
                    Point::new(3, 4),
                    Point::new(3, 10),
                    Point::new(0, 10),
                ]),
                vec![],
            )
            .unwrap(),
        ];

        for polygon in fixtures {
            let ordinary = GeneralPolygonPairwiseEnumerator
                .enumerate(&polygon)
                .unwrap();
            let boundary = Boundary::from_polygon(&polygon);
            let reflex = boundary
                .reflex_vertices
                .iter()
                .map(|vertex| vertex.point)
                .collect::<BTreeSet<_>>();
            let formal = FormalRectilinearPolygon::new(polygon, Ornament::default()).unwrap();
            for vertex in formal.vertex_geometry().unwrap() {
                assert_eq!(
                    vertex.local_nonconvexity_measure,
                    u8::from(reflex.contains(&vertex.point)),
                    "local measure differs at {:?}",
                    vertex.point
                );
            }
            let formal_chords = formal.effective_chords_pairwise().unwrap();
            assert_eq!(formal_chords.horizontal, ordinary.horizontal);
            assert_eq!(formal_chords.vertical, ordinary.vertical);
            let source = formal.effective_chords_source().unwrap();
            let sweep = SoltanGorpinevichSweepEnumerator
                .enumerate(formal.region())
                .unwrap();
            assert_eq!(source.families.horizontal, ordinary.horizontal);
            assert_eq!(source.families.vertical, ordinary.vertical);
            assert_eq!(source.families.horizontal, sweep.horizontal);
            assert_eq!(source.families.vertical, sweep.vertical);
        }
    }

    #[test]
    fn polygon_chords_accept_only_axis_aligned_pairs() {
        let polygon = first_grid_derived_polygon_with_chords();
        let families = GeneralPolygonPairwiseEnumerator
            .enumerate(&polygon)
            .unwrap();
        assert!(!families.horizontal.is_empty() || !families.vertical.is_empty());
        assert!(
            families
                .horizontal
                .iter()
                .all(|chord| chord.left() < chord.right())
        );
        assert!(
            families
                .vertical
                .iter()
                .all(|chord| chord.bottom() < chord.top())
        );
    }

    #[test]
    fn polygon_chords_reject_hole_interiors() {
        let polygon =
            RectilinearPolygon::new(rectangle(0, 0, 12, 10), vec![rectangle(4, 3, 8, 7)]).unwrap();
        let boundary = Boundary::from_polygon(&polygon);
        let boundary_index = BoundaryIndex::new(&boundary).unwrap();
        assert!(
            !horizontal_satisfies_definition_7(
                &polygon,
                &boundary,
                &boundary_index,
                2,
                10,
                5,
                &mut super::PolygonChordEnumerationMetrics::default(),
            )
            .unwrap()
        );
    }

    #[test]
    fn polygon_chords_require_reflex_collinear_endpoints() {
        let polygon = first_grid_derived_polygon_with_chords();
        let boundary = Boundary::from_polygon(&polygon);
        let boundary_index = BoundaryIndex::new(&boundary).unwrap();
        let reflex_points = boundary
            .reflex_vertices
            .iter()
            .map(|vertex| vertex.point)
            .collect::<std::collections::BTreeSet<_>>();
        let families = GeneralPolygonPairwiseEnumerator
            .enumerate(&polygon)
            .unwrap();
        for chord in &families.horizontal {
            for point in [
                Point::new(chord.left(), chord.y()),
                Point::new(chord.right(), chord.y()),
            ] {
                assert!(reflex_points.contains(&point));
                assert!(
                    endpoint_has_collinear_edge(&boundary, &boundary_index, point, true).unwrap()
                );
            }
        }
        for chord in &families.vertical {
            for point in [
                Point::new(chord.x(), chord.bottom()),
                Point::new(chord.x(), chord.top()),
            ] {
                assert!(reflex_points.contains(&point));
                assert!(
                    endpoint_has_collinear_edge(&boundary, &boundary_index, point, false).unwrap()
                );
            }
        }
    }

    #[test]
    fn polygon_chords_reject_nonvertex_boundary_crossings() {
        let polygon = RectilinearPolygon::new(rectangle(0, 0, 10, 10), vec![]).unwrap();
        let boundary = Boundary::from_polygon(&polygon);
        let boundary_index = BoundaryIndex::new(&boundary).unwrap();
        assert!(
            !horizontal_satisfies_definition_7(
                &polygon,
                &boundary,
                &boundary_index,
                -1,
                11,
                5,
                &mut super::PolygonChordEnumerationMetrics::default(),
            )
            .unwrap()
        );
    }

    #[test]
    fn grid_derived_polygon_chords_match_on_all_3x3_masks() {
        let enumerator = GeneralPolygonPairwiseEnumerator;
        let indexed = IndexedPolygonPairwiseEnumerator;
        let sweep = SoltanGorpinevichSweepEnumerator;
        let mut compared = 0;
        for mask in 1_u16..1 << 9 {
            let grid =
                ColorGrid::new(3, 3, (0..9).map(|bit| mask & (1 << bit) != 0).collect()).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = Boundary::from_component(&component).unwrap();
                let Ok(polygon) = boundary.to_polygon() else {
                    continue;
                };
                let grid_families = GridInteriorRunEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                let polygon_families = enumerator.enumerate(&polygon).unwrap();
                let prepared = PreparedPolygonContext::new(&polygon).unwrap();
                let indexed_result = indexed.enumerate_prepared(&prepared).unwrap();
                let sweep_result = sweep.enumerate_prepared(&prepared).unwrap();
                super::audit_sweep_provenance(&prepared, &sweep_result).unwrap();
                assert_eq!(grid_families.horizontal, polygon_families.horizontal);
                assert_eq!(grid_families.vertical, polygon_families.vertical);
                assert_eq!(
                    polygon_families.horizontal,
                    indexed_result.families.horizontal
                );
                assert_eq!(polygon_families.vertical, indexed_result.families.vertical);
                assert_eq!(
                    polygon_families.horizontal,
                    sweep_result.families.horizontal
                );
                assert_eq!(polygon_families.vertical, sweep_result.families.vertical);
                assert_eq!(
                    indexed_result
                        .metrics
                        .polygon_aligned_reflex_candidate_pairs,
                    prepared.metrics().polygon_aligned_reflex_candidate_pairs
                );
                assert_eq!(
                    indexed_result.metrics.polygon_unaligned_reflex_pair_checks,
                    0
                );
                assert_eq!(
                    indexed_result
                        .metrics
                        .polygon_definition7_full_boundary_scans,
                    0
                );
                assert_eq!(sweep_result.metrics.sweep_aligned_pair_iterations, 0);
                assert_eq!(sweep_result.metrics.sweep_all_pair_iterations, 0);
                assert_eq!(sweep_result.metrics.sweep_definition7_fallback_checks, 0);
                assert_eq!(sweep_result.metrics.sweep_full_boundary_scans, 0);
                assert_eq!(sweep_result.metrics.sweep_duplicate_output_count, 0);
                assert_eq!(
                    sweep_result.metrics.sweep_output_horizontal_chords,
                    sweep_result.families.horizontal.len()
                );
                assert_eq!(
                    sweep_result.metrics.sweep_output_vertical_chords,
                    sweep_result.families.vertical.len()
                );
                compared += 1;
            }
        }
        assert!(compared > 100);
    }

    #[test]
    fn indexed_polygon_chords_match_native_nonuniform_and_hole_fixtures() {
        let fixtures = [
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(1_000_000_000, 0),
                    Point::new(1_000_000_000, 17),
                    Point::new(41, 17),
                    Point::new(41, 23),
                    Point::new(1_000_000_000, 23),
                    Point::new(1_000_000_000, 40),
                    Point::new(0, 40),
                ]),
                vec![],
            )
            .unwrap(),
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(100, 0),
                    Point::new(100, 80),
                    Point::new(0, 80),
                ]),
                vec![rectangle(10, 10, 30, 25), rectangle(60, 35, 90, 70)],
            )
            .unwrap(),
            first_grid_derived_polygon_with_chords(),
        ];
        for polygon in fixtures {
            let reference = GeneralPolygonPairwiseEnumerator
                .enumerate(&polygon)
                .unwrap();
            let prepared = PreparedPolygonContext::new(&polygon).unwrap();
            let indexed = IndexedPolygonPairwiseEnumerator
                .enumerate_prepared(&prepared)
                .unwrap();
            let sweep = SoltanGorpinevichSweepEnumerator
                .enumerate_prepared(&prepared)
                .unwrap();
            super::audit_sweep_provenance(&prepared, &sweep).unwrap();
            assert_eq!(reference.horizontal, indexed.families.horizontal);
            assert_eq!(reference.vertical, indexed.families.vertical);
            assert_eq!(reference.horizontal, sweep.families.horizontal);
            assert_eq!(reference.vertical, sweep.families.vertical);
            assert_eq!(sweep.metrics.sweep_duplicate_output_count, 0);
            assert_eq!(sweep.metrics.sweep_aligned_pair_iterations, 0);
            assert_eq!(sweep.metrics.sweep_all_pair_iterations, 0);
            assert_eq!(sweep.metrics.sweep_definition7_fallback_checks, 0);
            assert_eq!(sweep.metrics.sweep_full_boundary_scans, 0);
        }
    }

    #[test]
    fn sweep_queries_all_four_ordinary_reflex_ray_directions() {
        let mut directions = std::collections::BTreeSet::new();
        for mask in 1_u16..1 << 9 {
            let grid =
                ColorGrid::new(3, 3, (0..9).map(|bit| mask & (1 << bit) != 0).collect()).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = Boundary::from_component(&component).unwrap();
                let Ok(polygon) = boundary.to_polygon() else {
                    continue;
                };
                let prepared = PreparedPolygonContext::new(&polygon).unwrap();
                for reflex in &prepared.boundary().reflex_vertices {
                    let id = prepared.boundary_index().vertex_id(reflex.point).unwrap();
                    for axis in [SweepAxis::Horizontal, SweepAxis::Vertical] {
                        directions.insert((
                            axis,
                            sweep_interior_direction(prepared.boundary(), id, axis).unwrap(),
                        ));
                    }
                }
            }
        }
        assert_eq!(
            directions,
            std::collections::BTreeSet::from([
                (SweepAxis::Horizontal, false),
                (SweepAxis::Horizontal, true),
                (SweepAxis::Vertical, false),
                (SweepAxis::Vertical, true),
            ])
        );
    }

    #[test]
    fn sweep_certificate_is_canonical_and_output_sized_for_ordinary_holes() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(30, 0),
                Point::new(30, 30),
                Point::new(0, 30),
            ]),
            vec![rectangle(4, 4, 10, 10), rectangle(16, 14, 24, 25)],
        )
        .unwrap();
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let reference = GeneralPolygonPairwiseEnumerator
            .enumerate_prepared(&prepared)
            .unwrap();
        let result = SoltanGorpinevichSweepEnumerator
            .enumerate_prepared(&prepared)
            .unwrap();
        assert_eq!(result.families.horizontal, reference.horizontal);
        assert_eq!(result.families.vertical, reference.vertical);
        let certificate = result.sweep_certificate.unwrap();
        assert!(
            certificate
                .event_summaries
                .iter()
                .all(|summary| summary.insert_query_remove_order)
        );
        let mut records = std::collections::BTreeSet::new();
        for record in &certificate.output_records {
            assert!(record.source_point < record.target_point);
            assert!(records.insert((record.axis, record.source, record.target)));
        }
        assert_eq!(
            certificate.output_records.len(),
            result.families.horizontal.len() + result.families.vertical.len()
        );
        assert_eq!(result.metrics.sweep_duplicate_output_count, 0);
    }

    #[test]
    fn sweep_certificate_bounds_event_trace_without_bounding_outputs() {
        let notch_count = 40_i64;
        let mut vertices = vec![Point::new(0, 0), Point::new(4 * notch_count + 4, 0)];
        vertices.push(Point::new(4 * notch_count + 4, 4));
        for index in (0..notch_count).rev() {
            let left = 4 * index + 1;
            vertices.extend([
                Point::new(left + 2, 4),
                Point::new(left + 2, 2),
                Point::new(left, 2),
                Point::new(left, 4),
            ]);
        }
        vertices.push(Point::new(0, 4));
        let polygon = RectilinearPolygon::new(OrthogonalLoop::new(vertices), vec![]).unwrap();
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let result = SoltanGorpinevichSweepEnumerator
            .enumerate_prepared(&prepared)
            .unwrap();
        let certificate = result.sweep_certificate.unwrap();
        assert!(certificate.event_trace_truncated);
        assert!(certificate.event_summaries.len() <= 2 * super::SWEEP_EVENT_TRACE_LIMIT);
        assert_eq!(
            certificate.output_records.len(),
            result.metrics.sweep_output_horizontal_chords
                + result.metrics.sweep_output_vertical_chords
        );
    }

    #[test]
    fn rejects_collinear_boundary_overlap_and_hole_interior() {
        let notch = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(8, 0),
                Point::new(8, 8),
                Point::new(5, 8),
                Point::new(5, 3),
                Point::new(3, 3),
                Point::new(3, 8),
                Point::new(0, 8),
            ]),
            vec![],
        )
        .unwrap();
        let families = GeneralPolygonPairwiseEnumerator.enumerate(&notch).unwrap();
        assert!(families.horizontal.is_empty());
        assert!(families.vertical.is_empty());

        let with_hole = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(12, 0),
                Point::new(12, 10),
                Point::new(0, 10),
            ]),
            vec![OrthogonalLoop::new(vec![
                Point::new(4, 3),
                Point::new(4, 7),
                Point::new(8, 7),
                Point::new(8, 3),
            ])],
        )
        .unwrap();
        let families = GeneralPolygonPairwiseEnumerator
            .enumerate(&with_hole)
            .unwrap();
        assert!(families.horizontal.is_empty());
        assert!(families.vertical.is_empty());
    }

    #[test]
    fn coordinate_completion_handles_large_gaps_without_rasterization() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(1_000_000_000, 0),
                Point::new(1_000_000_000, 10),
                Point::new(0, 10),
            ]),
            vec![],
        )
        .unwrap();
        let completion = super::CoordinateCompressedCompletion
            .complete(&polygon, &[], &[], &[], &[])
            .unwrap();
        assert_eq!(completion.rectangles.len(), 1);
        assert_eq!(completion.metrics.coordinate_compression_x_count, 2);
        assert_eq!(completion.metrics.coordinate_compression_y_count, 2);
        assert_eq!(completion.metrics.atomic_cell_count, 1);
    }

    #[test]
    fn coordinate_completion_dissects_an_l_shape() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 1),
                Point::new(1, 1),
                Point::new(1, 4),
                Point::new(0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let completion = super::CoordinateCompressedCompletion
            .complete(&polygon, &[], &[], &[], &[])
            .unwrap();
        assert_eq!(completion.rectangles.len(), 2);
        super::validate_polygon_dissection(&polygon, &completion.rectangles).unwrap();
    }

    #[test]
    fn indexed_completion_matches_reference_cuts_and_rectangles() {
        let fixtures = [
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(4, 1),
                    Point::new(1, 1),
                    Point::new(1, 4),
                    Point::new(0, 4),
                ]),
                vec![],
            )
            .unwrap(),
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(12, 0),
                    Point::new(12, 10),
                    Point::new(0, 10),
                ]),
                vec![rectangle(4, 3, 8, 7)],
            )
            .unwrap(),
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(8, 0),
                    Point::new(8, 8),
                    Point::new(5, 8),
                    Point::new(5, 3),
                    Point::new(3, 3),
                    Point::new(3, 8),
                    Point::new(0, 8),
                ]),
                vec![],
            )
            .unwrap(),
        ];
        for polygon in fixtures {
            let reference = CoordinateCompressedCompletion
                .complete(&polygon, &[], &[], &[], &[])
                .unwrap();
            let indexed = IndexedPolygonCompletion
                .complete(&polygon, &[], &[], &[], &[])
                .unwrap();
            assert_eq!(reference.rectangles, indexed.rectangles);
            assert_eq!(
                reference.added_horizontal_cuts,
                indexed.added_horizontal_cuts
            );
            assert_eq!(reference.added_vertical_cuts, indexed.added_vertical_cuts);
            assert_eq!(indexed.metrics.completion_global_candidate_rebuilds, 0);
            assert_eq!(indexed.metrics.completion_full_boundary_scans, 0);
            assert_eq!(indexed.metrics.completion_full_cut_scans, 0);
        }
    }

    #[test]
    fn indexed_completion_matches_reference_on_all_3x3_polygons() {
        let mut compared = 0;
        for mask in 1_u16..1 << 9 {
            let grid =
                ColorGrid::new(3, 3, (0..9).map(|bit| mask & (1 << bit) != 0).collect()).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = Boundary::from_component(&component).unwrap();
                let Ok(polygon) = boundary.to_polygon() else {
                    continue;
                };
                let families = GeneralPolygonPairwiseEnumerator
                    .enumerate(&polygon)
                    .unwrap();
                let selected_horizontal = vec![true; families.horizontal.len()];
                let selected_vertical = vec![false; families.vertical.len()];
                let reference = CoordinateCompressedCompletion
                    .complete(
                        &polygon,
                        &families.horizontal,
                        &families.vertical,
                        &selected_horizontal,
                        &selected_vertical,
                    )
                    .unwrap();
                let prepared = PreparedPolygonContext::new(&polygon).unwrap();
                let indexed = IndexedPolygonCompletion
                    .complete_prepared(
                        &prepared,
                        &families.horizontal,
                        &families.vertical,
                        &selected_horizontal,
                        &selected_vertical,
                    )
                    .unwrap();
                assert_eq!(
                    reference.selected_horizontal_cuts,
                    indexed.selected_horizontal_cuts
                );
                assert_eq!(
                    reference.selected_vertical_cuts,
                    indexed.selected_vertical_cuts
                );
                assert_eq!(
                    reference.added_horizontal_cuts,
                    indexed.added_horizontal_cuts
                );
                assert_eq!(reference.added_vertical_cuts, indexed.added_vertical_cuts);
                assert_eq!(reference.rectangles, indexed.rectangles);
                assert_eq!(indexed.metrics.completion_global_candidate_rebuilds, 0);
                assert_eq!(indexed.metrics.completion_full_boundary_scans, 0);
                assert_eq!(indexed.metrics.completion_full_cut_scans, 0);
                compared += 1;
            }
        }
        assert!(compared > 100);
    }

    #[test]
    fn dynamic_cut_index_reports_new_intersections() {
        let horizontal = HorizontalCutSegment::new(1, 9, 5).unwrap();
        let vertical = VerticalCutSegment::new(5, 1, 9).unwrap();
        let mut index = crate::polygon_cut_index::oracle::Index::default();
        assert!(index.insert_horizontal_with_intersections(horizontal).0);
        let (inserted, intersections) = index.insert_vertical_with_intersections(vertical);
        assert!(inserted);
        assert_eq!(intersections, vec![Point::new(5, 5)]);
        assert!(index.contains_horizontal_ray(Point::new(2, 5), true));
        assert!(index.contains_vertical_ray(Point::new(5, 2), true));
        assert_eq!(index.horizontal_segments(), vec![horizontal]);
        assert_eq!(index.vertical_segments(), vec![vertical]);
    }

    #[test]
    fn arrangement_validators_match_on_valid_and_invalid_rectangles() {
        let polygon = RectilinearPolygon::new(rectangle(0, 0, 4, 4), vec![]).unwrap();
        let horizontal_cuts = std::collections::BTreeSet::from([
            HorizontalCutSegment {
                left: 0,
                right: 4,
                y: 2,
            },
            HorizontalCutSegment {
                left: 0,
                right: 4,
                y: 3,
            },
        ]);
        let vertical_cuts = std::collections::BTreeSet::from([VerticalCutSegment {
            x: 2,
            bottom: 0,
            top: 4,
        }]);
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let arrangement =
            polygon_arrangement::Arrangement::new(&prepared, &horizontal_cuts, &vertical_cuts)
                .unwrap();
        let valid = vec![CoordinateRect::new(0, 0, 4, 4).unwrap()];
        assert!(
            polygon_arrangement::oracle::Validator
                .validate(&polygon, &valid)
                .is_ok()
        );
        assert!(
            polygon_arrangement::experiment::Validator
                .validate(&arrangement, &polygon, &valid)
                .is_ok()
        );

        let overlap = vec![
            CoordinateRect::new(0, 0, 4, 3).unwrap(),
            CoordinateRect::new(0, 2, 2, 4).unwrap(),
        ];
        let reference = polygon_arrangement::oracle::Validator
            .validate(&polygon, &overlap)
            .unwrap_err();
        let indexed = polygon_arrangement::experiment::Validator
            .validate(&arrangement, &polygon, &overlap)
            .unwrap_err();
        assert_eq!(reference, indexed);
    }
}
