use super::{Backend, PolygonError, RectilinearPolygon};

/// Definition-level quadratic polygon validator.
#[derive(Clone, Copy, Debug, Default)]
pub struct Validator;

impl super::Validator for Validator {
    fn validate(&self, polygon: &RectilinearPolygon) -> Result<(), PolygonError> {
        polygon.validate()
    }

    fn name(&self) -> &'static str {
        Backend::Oracle.name()
    }
}
