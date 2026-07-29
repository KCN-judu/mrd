//! Orthogonal intersection backends for sparse subdivision construction.

use std::collections::BTreeSet;

use rect_core::Point;
use serde::{Deserialize, Serialize};

pub mod experiment;
mod graph;
pub mod oracle;

use graph::Segment;
pub use graph::{
    AtomicSegment, Face, FaceId, Graph, HalfEdge, HalfEdgeId, Metrics, Vertex, VertexId,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Backend {
    /// Horizontal-range scan used as a correctness oracle.
    #[serde(rename = "reference-range-scan")]
    Oracle,
    /// Output-sensitive closed-endpoint x sweep.
    #[default]
    #[serde(rename = "orthogonal-sweep")]
    Experiment,
}

impl Backend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oracle => "reference-range-scan",
            Self::Experiment => "orthogonal-sweep",
        }
    }
}

fn split(backend: Backend, segments: &[Segment]) -> (Vec<BTreeSet<i64>>, BTreeSet<Point>, Metrics) {
    match backend {
        Backend::Oracle => oracle::split(segments),
        Backend::Experiment => experiment::split(segments),
    }
}

#[cfg(test)]
mod tests {
    use super::Backend;

    #[test]
    fn backend_evidence_names_remain_stable() {
        assert_eq!(
            serde_json::to_string(&Backend::Oracle).unwrap(),
            "\"reference-range-scan\""
        );
        assert_eq!(
            serde_json::to_string(&Backend::Experiment).unwrap(),
            "\"orthogonal-sweep\""
        );
        assert_eq!(Backend::Oracle.name(), "reference-range-scan");
        assert_eq!(Backend::Experiment.name(), "orthogonal-sweep");
    }
}
