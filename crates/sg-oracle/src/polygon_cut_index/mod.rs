//! Exact cut indexes for the polygon completion frontier.

use std::collections::BTreeSet;

use mrd_domain::{MemoryEstimate, Point};
use serde::{Deserialize, Serialize};

use crate::polygon::{HorizontalCutSegment, PolygonDirection, PolygonSgError, VerticalCutSegment};

pub mod experiment;
pub mod oracle;

/// Selects the mutable cut index used by indexed polygon completion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Backend {
    /// Map-of-lines correctness oracle.
    #[serde(rename = "reference-line-maps")]
    Oracle,
    /// Coordinate-compressed interval stabbing index with no line scans.
    #[default]
    #[serde(rename = "dynamic-stabbing")]
    Experiment,
}

impl Backend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oracle => "line-map-reference",
            Self::Experiment => "dynamic-stabbing",
        }
    }
}

/// Work performed by a mutable cut index.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub insertions: usize,
    pub canonical_node_insertions: usize,
    pub stabbing_queries: usize,
    pub tree_node_visits: usize,
    pub ordered_set_queries: usize,
    pub reported_intersections: usize,
    pub coordinate_line_scans: usize,
    pub interval_scans: usize,
    pub logical_tree_node_count: usize,
    pub materialized_tree_node_count: usize,
    pub ordered_set_entry_count: usize,
    pub owned_bytes: usize,
    pub memory_estimate: MemoryEstimate,
}

#[derive(Clone, Debug)]
pub(crate) enum Index {
    Oracle {
        index: oracle::Index,
        metrics: Metrics,
    },
    Experiment(Box<experiment::Index>),
}

impl Index {
    pub(crate) fn new(
        backend: Backend,
        coordinates: BTreeSet<i64>,
    ) -> Result<Self, PolygonSgError> {
        match backend {
            Backend::Oracle => Ok(Self::Oracle {
                index: oracle::Index::default(),
                metrics: Metrics::default(),
            }),
            Backend::Experiment => Ok(Self::Experiment(Box::new(experiment::Index::new(
                coordinates,
            )?))),
        }
    }

    pub(crate) fn contains_horizontal_ray(&mut self, point: Point, east: bool) -> bool {
        match self {
            Self::Oracle { index, metrics } => {
                metrics.ordered_set_queries += 1;
                index.contains_horizontal_ray(point, east)
            }
            Self::Experiment(index) => index.contains_horizontal_ray(point, east),
        }
    }

    pub(crate) fn contains_vertical_ray(&mut self, point: Point, north: bool) -> bool {
        match self {
            Self::Oracle { index, metrics } => {
                metrics.ordered_set_queries += 1;
                index.contains_vertical_ray(point, north)
            }
            Self::Experiment(index) => index.contains_vertical_ray(point, north),
        }
    }

    pub(crate) fn insert_horizontal_with_intersections(
        &mut self,
        segment: HorizontalCutSegment,
    ) -> Result<(bool, Vec<Point>), PolygonSgError> {
        match self {
            Self::Oracle { index, metrics } => {
                metrics.insertions += 1;
                metrics.coordinate_line_scans += 1;
                Ok(index.insert_horizontal_with_intersections(segment))
            }
            Self::Experiment(index) => index.insert_horizontal_with_intersections(segment),
        }
    }

    pub(crate) fn insert_vertical_with_intersections(
        &mut self,
        segment: VerticalCutSegment,
    ) -> Result<(bool, Vec<Point>), PolygonSgError> {
        match self {
            Self::Oracle { index, metrics } => {
                metrics.insertions += 1;
                metrics.coordinate_line_scans += 1;
                Ok(index.insert_vertical_with_intersections(segment))
            }
            Self::Experiment(index) => index.insert_vertical_with_intersections(segment),
        }
    }

    pub(crate) fn nearest_blocker(
        &mut self,
        point: Point,
        direction: PolygonDirection,
    ) -> Option<Point> {
        match self {
            Self::Oracle { index, metrics } => {
                metrics.stabbing_queries += 1;
                metrics.coordinate_line_scans += 1;
                index.nearest_blocker(point, direction)
            }
            Self::Experiment(index) => index.nearest_blocker(point, direction),
        }
    }

    #[must_use]
    pub(crate) fn horizontal_segments(&self) -> Vec<HorizontalCutSegment> {
        match self {
            Self::Oracle { index, .. } => index.horizontal_segments(),
            Self::Experiment(index) => index.horizontal_segments(),
        }
    }

    #[must_use]
    pub(crate) fn vertical_segments(&self) -> Vec<VerticalCutSegment> {
        match self {
            Self::Oracle { index, .. } => index.vertical_segments(),
            Self::Experiment(index) => index.vertical_segments(),
        }
    }

    #[must_use]
    pub(crate) fn metrics(&self) -> Metrics {
        match self {
            Self::Oracle { metrics, .. } => metrics.clone(),
            Self::Experiment(index) => index.metrics(),
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
            "\"reference-line-maps\""
        );
        assert_eq!(
            serde_json::to_string(&Backend::Experiment).unwrap(),
            "\"dynamic-stabbing\""
        );
        assert_eq!(Backend::Oracle.name(), "line-map-reference");
        assert_eq!(Backend::Experiment.name(), "dynamic-stabbing");
    }
}
