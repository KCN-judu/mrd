//! Final-geometry recovery selection.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Backend {
    /// Coordinate-compressed flood-fill oracle.
    #[serde(rename = "dense-coordinate-arrangement")]
    Oracle,
    /// Sparse half-edge subdivision and face walk.
    #[default]
    #[serde(rename = "sparse-subdivision")]
    Experiment,
    /// Selects one backend from cheap coordinate and segment estimates.
    #[serde(rename = "auto")]
    Auto,
}

impl Backend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oracle => "dense-arrangement",
            Self::Experiment => "sparse-subdivision",
            Self::Auto => "auto",
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
            "\"dense-coordinate-arrangement\""
        );
        assert_eq!(
            serde_json::to_string(&Backend::Experiment).unwrap(),
            "\"sparse-subdivision\""
        );
        assert_eq!(serde_json::to_string(&Backend::Auto).unwrap(), "\"auto\"");
        assert_eq!(Backend::Oracle.name(), "dense-arrangement");
        assert_eq!(Backend::Experiment.name(), "sparse-subdivision");
        assert_eq!(Backend::Auto.name(), "auto");
    }
}
