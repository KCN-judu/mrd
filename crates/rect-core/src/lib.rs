pub mod boundary;
pub mod context;
pub mod formats;
pub mod geometry;
pub mod grid;
pub mod result;
pub mod validation;

pub use boundary::{
    Boundary, BoundaryError, BoundaryIndex, BoundaryIndexError, BoundaryLoop, BoundaryLoopId,
    BoundaryVertexId, ReflexVertex,
};
pub use context::{PreparedComponentContext, PreparedContextError};
pub use formats::{FormatError, SvgOverlay, render_dissection_svg};
pub use geometry::{
    BicliqueId, ChordId, Coord, GeometryError, GridRect, HorizontalChord, HorizontalChordId, Point,
    Segment, VerticalChord, VerticalChordId, closed_chords_intersect,
};
pub use grid::{Cell, ColorGrid, ComponentId, GridComponent, GridError, PreparedGridComponent};
pub use result::{Certificate, Diagnostics, DissectionResult, ExactRatio, ExecutionTrace};
pub use validation::{ValidationError, validate_dissection, validate_dissection_prepared};
