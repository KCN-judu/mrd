use super::{Backend, PolygonError, RectilinearPolygon};

/// Deterministic exact orthogonal range-sweep validator.
#[derive(Clone, Copy, Debug, Default)]
pub struct Validator;

impl super::Validator for Validator {
    fn validate(&self, polygon: &RectilinearPolygon) -> Result<(), PolygonError> {
        crate::polygon_index::validate_polygon_sweep(polygon)
    }

    fn name(&self) -> &'static str {
        Backend::Experiment.name()
    }
}
