pub mod boundary;
pub mod formats;
pub mod geometry;
pub mod grid;
pub mod result;
pub mod validation;

pub use boundary::{Boundary, BoundaryError, BoundaryLoop, ReflexVertex};
pub use formats::{FormatError, SvgOverlay, render_dissection_svg};
pub use geometry::{
    BicliqueId, ChordId, Coord, GeometryError, GridRect, HorizontalChord, HorizontalChordId, Point,
    Segment, VerticalChord, VerticalChordId, closed_chords_intersect,
};
pub use grid::{Cell, ColorGrid, ComponentId, GridComponent, GridError};
pub use result::{Certificate, Diagnostics, DissectionResult, ExactRatio};
pub use validation::{ValidationError, validate_dissection};
