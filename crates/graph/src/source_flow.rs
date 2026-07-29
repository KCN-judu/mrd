//! Experimental source-shaped flow backend boundary.
//!
//! This module intentionally has no Dinic, Push--Relabel, or enumerating
//! min-cost-cycle dependency. It cannot be enabled until P9.5 supplies the
//! complete certified iteration and exact recovery path.

use thiserror::Error;

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
}

/// The source-shaped backend is not complete enough to execute.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// Certified iteration and recovery integration has not been implemented.
    #[error("source-shaped flow backend is not yet complete")]
    Incomplete,
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
