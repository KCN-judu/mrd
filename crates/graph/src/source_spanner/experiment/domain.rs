//! Shared finite-domain inputs and failures for exhaustive experiments.

use super::certificate::MAX_EXHAUSTIVE_NODES;

/// A finite graph domain in which every nontrivial cut is checked exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExhaustiveDomain {
    pub maximum_nodes: usize,
}

impl ExhaustiveDomain {
    pub(super) fn contains(self, nodes: usize) -> bool {
        self.maximum_nodes <= MAX_EXHAUSTIVE_NODES && (2..=self.maximum_nodes).contains(&nodes)
    }
}

/// A failure to build or verify an exhaustive experiment certificate.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("experiment input is outside the exhaustive certified domain")]
    OutsideCertifiedDomain,
    #[error("experiment degree does not satisfy its required sandwich")]
    DegreeSandwichViolation,
    #[error("experiment expansion certificate is invalid")]
    InvalidCertificate,
    #[error("experiment arithmetic overflowed")]
    Overflow,
    #[error("experiment graph model is invalid: {0}")]
    Model(#[source] super::super::model::Error),
}
