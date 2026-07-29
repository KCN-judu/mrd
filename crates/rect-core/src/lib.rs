pub mod boundary;
pub mod context;
pub mod formal_polygon;
pub mod formats;
pub mod geometry;
pub mod grid;
pub mod polygon;
pub mod polygon_index;
pub mod result;
pub mod validation;

pub use boundary::{
    Boundary, BoundaryError, BoundaryIndex, BoundaryIndexError, BoundaryLoop, BoundaryLoopId,
    BoundaryVertexId, ReflexVertex,
};
pub use context::{PreparedComponentContext, PreparedContextError};
pub use formal_polygon::{
    ElementarySegment, ElementarySegmentId, FormalBoundaryComponent, FormalBoundaryComponentId,
    FormalBoundaryComponentKind, FormalBoundaryDimension, FormalBoundaryIncidence,
    FormalBoundarySource, FormalChordAxis, FormalChordConstructionMetrics,
    FormalChordConstructionRecord, FormalChordConstructionResult, FormalChordEndpoints,
    FormalDirection, FormalEffectiveChordFamilies, FormalInnerAngle, FormalPolygonError,
    FormalQuadrant, FormalRectilinearPolygon, FormalVertex, FormalVertexGeometry, FormalVertexId,
    Ornament, OrnamentSegment,
};
pub use formats::{FormatError, SvgOverlay, render_dissection_svg, render_polygon_dissection_svg};
pub use geometry::{
    BicliqueId, ChordId, Coord, CoordinateRect, DoubledPoint, GeometryError, GridRect,
    HorizontalChord, HorizontalChordId, Point, Segment, VerticalChord, VerticalChordId,
    closed_chords_intersect,
};
pub use grid::{Cell, ColorGrid, ComponentId, GridComponent, GridError, PreparedGridComponent};
pub use polygon::{
    OrthogonalLoop, PolygonError, PolygonLoopId, PolygonVertexId, RectilinearDomain,
    RectilinearPolygon,
};
pub use polygon_index::{
    IndexedBoundaryEdge, OrthogonalDirection, OrthogonalEdgeIndex, PolygonErrorCategory,
    PolygonGeometryBackend, PolygonPreparationMetrics, PreparedPolygonContext,
    PreparedPolygonError,
};
pub use result::{
    Certificate, Diagnostics, DissectionResult, ExactRatio, ExecutionTrace, MemoryEstimate,
    PolygonDissectionResult,
};
pub use validation::{ValidationError, validate_dissection, validate_dissection_prepared};
