//! Sparse vertical slab validation backends.

use rect_core::{CoordinateRect, MemoryEstimate, RectilinearPolygon};
use serde::{Deserialize, Serialize};

use crate::polygon::PolygonValidationError;

pub mod experiment;
pub mod oracle;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Backend {
    /// Slab rescan used as a correctness oracle.
    #[serde(rename = "reference-slab-rescan")]
    Oracle,
    /// Event-driven y segment tree.
    #[default]
    #[serde(rename = "event-segment-tree")]
    Experiment,
}

impl Backend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oracle => "reference-slab-rescan",
            Self::Experiment => "event-segment-tree",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub validator_backend: String,
    pub x_event_count: usize,
    pub y_coordinate_count: usize,
    pub range_add_count: usize,
    pub parity_toggle_count: usize,
    pub segment_tree_node_visits: usize,
    pub root_checks: usize,
    pub boundary_edge_scans: usize,
    pub active_rectangle_resorts: usize,
    pub slab_count: usize,
    pub polygon_interval_events: usize,
    pub rectangle_interval_events: usize,
    pub owned_bytes: usize,
    pub memory_estimate: MemoryEstimate,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Validator;

impl Validator {
    /// # Errors
    ///
    /// Returns the first exact geometry, coverage, or area error.
    pub fn validate(
        self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<Metrics, PolygonValidationError> {
        experiment::validate(polygon, rectangles)
    }

    /// # Errors
    ///
    /// Returns the first exact geometry, coverage, or area error.
    pub fn validate_with_backend(
        self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
        backend: Backend,
    ) -> Result<Metrics, PolygonValidationError> {
        match backend {
            Backend::Oracle => oracle::validate(polygon, rectangles),
            Backend::Experiment => experiment::validate(polygon, rectangles),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Backend;

    #[test]
    fn backend_evidence_names_remain_stable() {
        assert_eq!(
            serde_json::to_string(&Backend::Oracle).unwrap(),
            "\"reference-slab-rescan\""
        );
        assert_eq!(
            serde_json::to_string(&Backend::Experiment).unwrap(),
            "\"event-segment-tree\""
        );
        assert_eq!(Backend::Oracle.name(), "reference-slab-rescan");
        assert_eq!(Backend::Experiment.name(), "event-segment-tree");
    }
}
