use dominance::{
    embedding::EmbeddingCoordinateBackend,
    experiment::{Verification, solve_with_verification_mode_and_embedding_backend},
};
use mrd_domain::ColorGrid;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::benchmark::BenchmarkContext;

/// Serializable finite-grid evidence for the direct parity embedding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub metadata: BenchmarkContext,
    pub masks_examined: usize,
    pub components_examined: usize,
    pub pipeline_comparisons: usize,
    pub mismatches: Vec<String>,
    pub solver_errors: Vec<String>,
    pub direct_rank_sort_count: usize,
    pub direct_rank_map_entry_count: usize,
    pub direct_rank_map_owned_bytes: usize,
    pub ranked_rank_sort_count: usize,
    pub ranked_rank_map_entry_count: usize,
    pub ranked_rank_map_owned_bytes: usize,
    pub direct_embedding_microseconds: u128,
    pub ranked_embedding_microseconds: u128,
    pub mode_baselines: BTreeMap<String, ModeBaseline>,
    pub performance_boundary: String,
}

/// Aggregate phase observations for one verification mode and coordinate backend.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModeBaseline {
    pub comparisons: usize,
    pub direct_phase_microseconds: BTreeMap<String, u128>,
    pub ranked_phase_microseconds: BTreeMap<String, u128>,
}

impl Evidence {
    #[must_use]
    pub fn verified(&self) -> bool {
        self.mismatches.is_empty()
            && self.solver_errors.is_empty()
            && self.direct_rank_sort_count == 0
            && self.direct_rank_map_entry_count == 0
            && self.direct_rank_map_owned_bytes == 0
    }
}

/// Runs the release evidence campaign for every nonempty 3x3 grid component.
///
/// # Panics
///
/// Panics only if the fixed 3x3 mask construction violates `ColorGrid`'s
/// length invariant, which would indicate an internal programming error.
#[must_use]
pub fn exhaustive_three_by_three(context: BenchmarkContext) -> Evidence {
    let mut evidence = Evidence {
        metadata: context,
        masks_examined: (1 << 9) - 1,
        components_examined: 0,
        pipeline_comparisons: 0,
        mismatches: Vec::new(),
        solver_errors: Vec::new(),
        direct_rank_sort_count: 0,
        direct_rank_map_entry_count: 0,
        direct_rank_map_owned_bytes: 0,
        ranked_rank_sort_count: 0,
        ranked_rank_map_entry_count: 0,
        ranked_rank_map_owned_bytes: 0,
        direct_embedding_microseconds: 0,
        ranked_embedding_microseconds: 0,
        mode_baselines: BTreeMap::new(),
        performance_boundary: "Embedding-phase timings are local observations, not a portable end-to-end speed claim. The deterministic benefit is zero ranked-coordinate sorts, map entries, and map-owned bytes on the direct finite-grid path.".to_owned(),
    };
    for mask in 1_u16..(1_u16 << 9) {
        let grid = ColorGrid::new(3, 3, (0..9).map(|bit| mask & (1 << bit) != 0).collect())
            .expect("fixed 3x3 mask dimensions are valid");
        for component in grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
        {
            evidence.components_examined += 1;
            for mode in [Verification::FullyAudited, Verification::CompactOnly] {
                let ranked = solve_with_verification_mode_and_embedding_backend(
                    &component,
                    mode,
                    EmbeddingCoordinateBackend::RankedCoordinates,
                );
                let direct = solve_with_verification_mode_and_embedding_backend(
                    &component,
                    mode,
                    EmbeddingCoordinateBackend::DirectGridParity,
                );
                let label = format!("mask-{mask}-{mode:?}");
                match (ranked, direct) {
                    (Ok(ranked), Ok(direct)) => {
                        evidence.pipeline_comparisons += 1;
                        if !record_metrics(&mut evidence, mode, &ranked, &direct)
                            || !results_match(&ranked, &direct)
                        {
                            evidence.mismatches.push(label);
                        }
                    }
                    (ranked, direct) => evidence
                        .solver_errors
                        .push(format!("{label}: ranked={ranked:?}; direct={direct:?}")),
                }
            }
        }
    }
    evidence
}

fn record_metrics(
    evidence: &mut Evidence,
    mode: Verification,
    ranked: &mrd_domain::DissectionResult,
    direct: &mrd_domain::DissectionResult,
) -> bool {
    let Some(direct_sorts) = direct.diagnostics.rank_sort_count else {
        return false;
    };
    let Some(direct_entries) = direct.diagnostics.rank_map_entry_count else {
        return false;
    };
    let Some(direct_bytes) = direct.diagnostics.rank_map_owned_bytes else {
        return false;
    };
    let Some(ranked_sorts) = ranked.diagnostics.rank_sort_count else {
        return false;
    };
    let Some(ranked_entries) = ranked.diagnostics.rank_map_entry_count else {
        return false;
    };
    let Some(ranked_bytes) = ranked.diagnostics.rank_map_owned_bytes else {
        return false;
    };
    evidence.direct_rank_sort_count += direct_sorts;
    evidence.direct_rank_map_entry_count += direct_entries;
    evidence.direct_rank_map_owned_bytes += direct_bytes;
    evidence.ranked_rank_sort_count += ranked_sorts;
    evidence.ranked_rank_map_entry_count += ranked_entries;
    evidence.ranked_rank_map_owned_bytes += ranked_bytes;
    evidence.direct_embedding_microseconds += direct
        .diagnostics
        .phase_microseconds
        .get("dominance_embedding")
        .copied()
        .unwrap_or(0);
    evidence.ranked_embedding_microseconds += ranked
        .diagnostics
        .phase_microseconds
        .get("dominance_embedding")
        .copied()
        .unwrap_or(0);
    let baseline = evidence
        .mode_baselines
        .entry(verification_mode_name(mode).to_owned())
        .or_default();
    baseline.comparisons += 1;
    accumulate_phase_times(
        &mut baseline.direct_phase_microseconds,
        &direct.diagnostics.phase_microseconds,
    );
    accumulate_phase_times(
        &mut baseline.ranked_phase_microseconds,
        &ranked.diagnostics.phase_microseconds,
    );
    true
}

fn accumulate_phase_times(target: &mut BTreeMap<String, u128>, phases: &BTreeMap<String, u128>) {
    for (phase, microseconds) in phases {
        *target.entry(phase.clone()).or_default() += microseconds;
    }
}

const fn verification_mode_name(mode: Verification) -> &'static str {
    match mode {
        Verification::FullyAudited => "fully-audited",
        Verification::CompactOnly => "compact-only",
    }
}

fn results_match(
    ranked: &mrd_domain::DissectionResult,
    direct: &mrd_domain::DissectionResult,
) -> bool {
    ranked.optimum_rectangle_count == direct.optimum_rectangle_count
        && ranked.rectangles == direct.rectangles
        && [
            "biclique_partition",
            "flow_value",
            "compressed_network_vertex_count",
            "compressed_network_arc_count",
            "internal_capacity",
            "internal_cut_arc_count",
            "min_cut_source_side",
            "cover_left",
            "cover_right",
            "selected_horizontal",
            "selected_vertical",
        ]
        .into_iter()
        .all(|field| certificate_field(ranked, field) == certificate_field(direct, field))
}

fn certificate_field<'a>(
    result: &'a mrd_domain::DissectionResult,
    field: &str,
) -> Option<&'a serde_json::Value> {
    result.certificate.as_ref()?.payload.get(field)
}
