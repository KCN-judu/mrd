//! Definition-level polygon dissection validation.

use mrd_domain::{CoordinateRect, RectilinearPolygon};

use crate::polygon::PolygonValidationError;

#[derive(Clone, Copy, Debug, Default)]
pub struct Validator;

impl Validator {
    /// # Errors
    ///
    /// Returns the first exact coverage failure.
    pub fn validate(
        self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<(), PolygonValidationError> {
        crate::polygon::validate_polygon_dissection(polygon, rectangles)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "reference-arrangement-scan"
    }
}
