use serde::{Deserialize, Serialize};

use super::model::{Problem, Run};
use crate::source_an19::petal::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[serde(rename = "exact_oracle")]
    Oracle,
    #[serde(rename = "reduced_exact")]
    Experiment,
    ProvedUnavailable,
}

pub trait Backend {
    fn kind(&self) -> Kind;

    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, trace-consistency, or unsupported
    /// backend error.
    fn run(&self, problem: &Problem<'_>) -> Result<Run, Error>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Unavailable;

impl Backend for Unavailable {
    fn kind(&self) -> Kind {
        Kind::ProvedUnavailable
    }

    fn run(&self, _problem: &Problem<'_>) -> Result<Run, Error> {
        Err(Error::UnprovedEventEngine)
    }
}
