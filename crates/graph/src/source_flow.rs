//! Experimental source-shaped flow backend boundary.
//!
//! This module intentionally has no Dinic, Push--Relabel, or enumerating
//! min-cost-cycle dependency. It cannot be enabled until P9.5 supplies the
//! complete certified iteration and exact recovery path.

use thiserror::Error;

use crate::{
    CertifiedIpmError, CertifiedIpmSnapshot, CirculationNetwork, IpmTerminationCertificate,
};

/// Explicit status for the experimental source-shaped backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status {
    /// This backend never carries the almost-linear runtime claim at present.
    pub an19_runtime_verified: bool,
}

/// Production entry point reserved for the complete P9.5 integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Backend;

impl Backend {
    /// Reports the conservative runtime status of this backend boundary.
    #[must_use]
    pub const fn status(self) -> Status {
        Status {
            an19_runtime_verified: false,
        }
    }

    /// Rejects use until P9.5 connects certified IPM iterations and recovery.
    ///
    /// # Errors
    ///
    /// Always returns an explicit unavailable error; no fallback is selected.
    pub const fn require_complete(self) -> Result<(), Error> {
        Err(Error::Incomplete)
    }

    /// Certifies the additive-half boundary without invoking any recovery Oracle.
    ///
    /// # Errors
    ///
    /// Returns the certified-IPM error when the supplied snapshot has not
    /// reached the additive-half termination boundary.
    pub fn certify_termination(
        self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
    ) -> Result<IpmTerminationCertificate, Error> {
        snapshot
            .certify_additive_half_termination(network)
            .map_err(Error::Ipm)
    }
}

/// The source-shaped backend is not complete enough to execute.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// Certified iteration and recovery integration has not been implemented.
    #[error("source-shaped flow backend is not yet complete")]
    Incomplete,
    /// The certified IPM snapshot cannot establish its termination boundary.
    #[error("certified IPM termination failed: {0}")]
    Ipm(#[from] CertifiedIpmError),
}

#[cfg(test)]
mod tests {
    use super::{Backend, Error};

    #[test]
    fn never_silently_selects_a_fallback_backend() {
        let backend = Backend;
        assert!(!backend.status().an19_runtime_verified);
        assert_eq!(backend.require_complete(), Err(Error::Incomplete));
    }
}
