//! Final polygon-dissection validation selection.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Backend {
    /// Coordinate-compressed difference-array oracle.
    #[serde(rename = "dense-arrangement")]
    Oracle,
    /// Sparse vertical slab experiment.
    #[default]
    #[serde(rename = "sparse-slab")]
    Experiment,
}

impl Backend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oracle => "dense-arrangement",
            Self::Experiment => "sparse-slab",
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
            "\"dense-arrangement\""
        );
        assert_eq!(
            serde_json::to_string(&Backend::Experiment).unwrap(),
            "\"sparse-slab\""
        );
        assert_eq!(Backend::Oracle.name(), "dense-arrangement");
        assert_eq!(Backend::Experiment.name(), "sparse-slab");
    }
}
