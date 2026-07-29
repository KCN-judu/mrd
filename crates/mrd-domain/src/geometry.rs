use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Coord = i64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ChordId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HorizontalChordId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VerticalChordId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BicliqueId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Point {
    pub x: Coord,
    pub y: Coord,
}

impl Point {
    #[must_use]
    pub const fn new(x: Coord, y: Coord) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: Point,
    pub end: Point,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct HorizontalChord {
    id: HorizontalChordId,
    left: Coord,
    right: Coord,
    y: Coord,
}

impl HorizontalChord {
    /// # Errors
    ///
    /// Returns [`GeometryError::InvalidHorizontalChord`] unless `left < right`.
    pub fn new(
        id: HorizontalChordId,
        left: Coord,
        right: Coord,
        y: Coord,
    ) -> Result<Self, GeometryError> {
        if left >= right {
            return Err(GeometryError::InvalidHorizontalChord { left, right });
        }
        Ok(Self { id, left, right, y })
    }

    #[must_use]
    pub const fn id(self) -> HorizontalChordId {
        self.id
    }

    #[must_use]
    pub const fn left(self) -> Coord {
        self.left
    }

    #[must_use]
    pub const fn right(self) -> Coord {
        self.right
    }

    #[must_use]
    pub const fn y(self) -> Coord {
        self.y
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct VerticalChord {
    id: VerticalChordId,
    x: Coord,
    bottom: Coord,
    top: Coord,
}

impl VerticalChord {
    /// # Errors
    ///
    /// Returns [`GeometryError::InvalidVerticalChord`] unless `bottom < top`.
    pub fn new(
        id: VerticalChordId,
        x: Coord,
        bottom: Coord,
        top: Coord,
    ) -> Result<Self, GeometryError> {
        if bottom >= top {
            return Err(GeometryError::InvalidVerticalChord { bottom, top });
        }
        Ok(Self { id, x, bottom, top })
    }

    #[must_use]
    pub const fn id(self) -> VerticalChordId {
        self.id
    }

    #[must_use]
    pub const fn x(self) -> Coord {
        self.x
    }

    #[must_use]
    pub const fn bottom(self) -> Coord {
        self.bottom
    }

    #[must_use]
    pub const fn top(self) -> Coord {
        self.top
    }
}

#[must_use]
pub const fn closed_chords_intersect(horizontal: HorizontalChord, vertical: VerticalChord) -> bool {
    horizontal.left <= vertical.x
        && vertical.x <= horizontal.right
        && vertical.bottom <= horizontal.y
        && horizontal.y <= vertical.top
}

impl Segment {
    /// # Errors
    ///
    /// Returns [`GeometryError`] for zero-length or non-axis-aligned segments.
    pub fn new(start: Point, end: Point) -> Result<Self, GeometryError> {
        if start == end {
            return Err(GeometryError::ZeroLengthSegment);
        }
        if start.x != end.x && start.y != end.y {
            return Err(GeometryError::NonAxisAlignedSegment);
        }
        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        self.start.y == self.end.y
    }

    #[must_use]
    pub const fn is_vertical(self) -> bool {
        self.start.x == self.end.x
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct GridRect {
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
}

/// A positive-area axis-aligned rectangle in the native coordinate system.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CoordinateRect {
    pub x0: Coord,
    pub y0: Coord,
    pub x1: Coord,
    pub y1: Coord,
}

impl CoordinateRect {
    /// Creates a coordinate-native rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonPositiveRectangle`] unless both dimensions
    /// are positive.
    pub fn new(x0: Coord, y0: Coord, x1: Coord, y1: Coord) -> Result<Self, GeometryError> {
        if x0 >= x1 || y0 >= y1 {
            return Err(GeometryError::NonPositiveRectangle);
        }
        Ok(Self { x0, y0, x1, y1 })
    }

    #[must_use]
    pub const fn width(self) -> i128 {
        self.x1 as i128 - self.x0 as i128
    }

    #[must_use]
    pub const fn height(self) -> i128 {
        self.y1 as i128 - self.y0 as i128
    }

    #[must_use]
    pub const fn area(self) -> i128 {
        self.width() * self.height()
    }

    #[must_use]
    pub const fn contains_doubled_point_strict(self, point: DoubledPoint) -> bool {
        2 * (self.x0 as i128) < point.x
            && point.x < 2 * (self.x1 as i128)
            && 2 * (self.y0 as i128) < point.y
            && point.y < 2 * (self.y1 as i128)
    }
}

/// Exact point coordinates scaled by two, used for side probes and midpoints.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DoubledPoint {
    pub x: i128,
    pub y: i128,
}

impl DoubledPoint {
    #[must_use]
    pub const fn new(x: i128, y: i128) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn from_point(point: Point) -> Self {
        Self {
            x: 2 * point.x as i128,
            y: 2 * point.y as i128,
        }
    }
}

impl GridRect {
    /// # Errors
    ///
    /// Returns [`GeometryError::NonPositiveRectangle`] unless both dimensions
    /// are positive.
    pub fn new(x0: usize, y0: usize, x1: usize, y1: usize) -> Result<Self, GeometryError> {
        if x0 >= x1 || y0 >= y1 {
            return Err(GeometryError::NonPositiveRectangle);
        }
        Ok(Self { x0, y0, x1, y1 })
    }

    #[must_use]
    pub const fn width(self) -> usize {
        self.x1 - self.x0
    }

    #[must_use]
    pub const fn height(self) -> usize {
        self.y1 - self.y0
    }

    /// # Panics
    ///
    /// Panics if the rectangle is invalid or its area does not fit `usize`.
    #[must_use]
    pub fn area(self) -> usize {
        self.width()
            .checked_mul(self.height())
            .expect("valid grid rectangle area fits usize")
    }

    #[must_use]
    pub const fn contains_cell(self, x: usize, y: usize) -> bool {
        self.x0 <= x && x < self.x1 && self.y0 <= y && y < self.y1
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GeometryError {
    #[error("a segment must have distinct endpoints")]
    ZeroLengthSegment,
    #[error("a segment must be horizontal or vertical")]
    NonAxisAlignedSegment,
    #[error("a grid rectangle must have positive width and height")]
    NonPositiveRectangle,
    #[error("horizontal chord endpoints must satisfy left < right, got {left} and {right}")]
    InvalidHorizontalChord { left: Coord, right: Coord },
    #[error("vertical chord endpoints must satisfy bottom < top, got {bottom} and {top}")]
    InvalidVerticalChord { bottom: Coord, top: Coord },
    #[error("a coordinate conversion or arithmetic operation overflowed")]
    CoordinateOverflow,
}
