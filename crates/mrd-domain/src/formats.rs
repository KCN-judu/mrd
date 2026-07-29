use std::fmt::Write;

use thiserror::Error;

use crate::{
    Boundary, BoundaryError, DissectionResult, GridComponent, HorizontalChord,
    PolygonDissectionResult, RectilinearPolygon, VerticalChord,
};

const SCALE: usize = 32;
const MARGIN: usize = 16;
const COLORS: [&str; 8] = [
    "#1f77b4", "#e45756", "#2a9d8f", "#f2a541", "#6f4e7c", "#4c956c", "#d45087", "#577590",
];

pub struct SvgOverlay<'a> {
    pub horizontal_chords: &'a [HorizontalChord],
    pub vertical_chords: &'a [VerticalChord],
    pub selected_horizontal: &'a [bool],
    pub selected_vertical: &'a [bool],
}

impl SvgOverlay<'_> {
    #[must_use]
    pub const fn empty() -> SvgOverlay<'static> {
        SvgOverlay {
            horizontal_chords: &[],
            vertical_chords: &[],
            selected_horizontal: &[],
            selected_vertical: &[],
        }
    }
}

/// Renders diagnostic geometry without feeding values back into a solver.
///
/// # Errors
///
/// Returns [`FormatError`] when boundary extraction, dimensions, overlay
/// cardinalities, or string formatting are invalid.
pub fn render_dissection_svg<C>(
    component: &GridComponent<C>,
    result: &DissectionResult,
    overlay: &SvgOverlay<'_>,
) -> Result<String, FormatError> {
    if (!overlay.selected_horizontal.is_empty()
        && overlay.selected_horizontal.len() != overlay.horizontal_chords.len())
        || (!overlay.selected_vertical.is_empty()
            && overlay.selected_vertical.len() != overlay.vertical_chords.len())
    {
        return Err(FormatError::OverlayDimensionMismatch);
    }
    let boundary = Boundary::from_component(component)?;
    let width = component
        .grid_width
        .checked_mul(SCALE)
        .and_then(|value| value.checked_add(MARGIN * 2))
        .ok_or(FormatError::DimensionOverflow)?;
    let height = component
        .grid_height
        .checked_mul(SCALE)
        .and_then(|value| value.checked_add(MARGIN * 2))
        .ok_or(FormatError::DimensionOverflow)?;
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    )?;
    writeln!(
        svg,
        r##"<rect width="100%" height="100%" fill="#ffffff"/>"##
    )?;

    for cell in &component.cells {
        let x = MARGIN + cell.x * SCALE;
        let y = MARGIN + (component.grid_height - cell.y - 1) * SCALE;
        writeln!(
            svg,
            r##"<rect x="{x}" y="{y}" width="{SCALE}" height="{SCALE}" fill="#edf0f2"/>"##
        )?;
    }

    for (index, rectangle) in result.rectangles.iter().enumerate() {
        let x = MARGIN + rectangle.x0 * SCALE;
        let y = MARGIN + (component.grid_height - rectangle.y1) * SCALE;
        let rectangle_width = rectangle.width() * SCALE;
        let rectangle_height = rectangle.height() * SCALE;
        let color = COLORS[index % COLORS.len()];
        writeln!(
            svg,
            r#"<rect x="{x}" y="{y}" width="{rectangle_width}" height="{rectangle_height}" fill="{color}" fill-opacity="0.18" stroke="{color}" stroke-width="2"/>"#
        )?;
    }

    for (index, chord) in overlay.horizontal_chords.iter().enumerate() {
        let selected = overlay
            .selected_horizontal
            .get(index)
            .copied()
            .unwrap_or(false);
        let x1 = svg_x(chord.left(), SCALE, MARGIN)?;
        let x2 = svg_x(chord.right(), SCALE, MARGIN)?;
        let y = svg_y(chord.y(), component.grid_height, SCALE, MARGIN)?;
        chord_line(&mut svg, x1, y, x2, y, selected)?;
    }
    for (index, chord) in overlay.vertical_chords.iter().enumerate() {
        let selected = overlay
            .selected_vertical
            .get(index)
            .copied()
            .unwrap_or(false);
        let x = svg_x(chord.x(), SCALE, MARGIN)?;
        let y1 = svg_y(chord.bottom(), component.grid_height, SCALE, MARGIN)?;
        let y2 = svg_y(chord.top(), component.grid_height, SCALE, MARGIN)?;
        chord_line(&mut svg, x, y1, x, y2, selected)?;
    }

    for &(start, end) in &boundary.unit_edges {
        let x1 = svg_x(start.x, SCALE, MARGIN)?;
        let y1 = svg_y(start.y, component.grid_height, SCALE, MARGIN)?;
        let x2 = svg_x(end.x, SCALE, MARGIN)?;
        let y2 = svg_y(end.y, component.grid_height, SCALE, MARGIN)?;
        writeln!(
            svg,
            r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#20262b" stroke-width="3" stroke-linecap="square"/>"##
        )?;
    }
    for vertex in &boundary.reflex_vertices {
        let x = svg_x(vertex.point.x, SCALE, MARGIN)?;
        let y = svg_y(vertex.point.y, component.grid_height, SCALE, MARGIN)?;
        writeln!(
            svg,
            r##"<circle cx="{x}" cy="{y}" r="4" fill="#d62828" stroke="#ffffff" stroke-width="1"/>"##
        )?;
    }
    svg.push_str("</svg>\n");
    Ok(svg)
}

/// Renders a boundary-native polygon dissection without rasterization.
///
/// # Errors
///
/// Returns [`FormatError`] when the polygon has no vertices or SVG formatting
/// fails.
pub fn render_polygon_dissection_svg(
    polygon: &RectilinearPolygon,
    result: &PolygonDissectionResult,
) -> Result<String, FormatError> {
    let mut points = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().copied());
    let first = points.next().ok_or(FormatError::EmptyPolygon)?;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    let span_x = i128::from(max_x) - i128::from(min_x);
    let span_y = i128::from(max_y) - i128::from(min_y);
    let margin = span_x.max(span_y).checked_div(50).unwrap_or(0).max(1);
    let view_x = i128::from(min_x) - margin;
    let view_y = -i128::from(max_y) - margin;
    let width = span_x + 2 * margin;
    let height = span_y + 2 * margin;
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="720" viewBox="{view_x} {view_y} {width} {height}">"#
    )?;
    writeln!(
        svg,
        r##"<rect x="{view_x}" y="{view_y}" width="{width}" height="{height}" fill="#ffffff"/>"##
    )?;
    for (index, rectangle) in result.rectangles.iter().enumerate() {
        let color = COLORS[index % COLORS.len()];
        let x = rectangle.x0;
        let y = -i128::from(rectangle.y1);
        let rectangle_width = i128::from(rectangle.x1) - i128::from(rectangle.x0);
        let rectangle_height = i128::from(rectangle.y1) - i128::from(rectangle.y0);
        writeln!(
            svg,
            r#"<rect x="{x}" y="{y}" width="{rectangle_width}" height="{rectangle_height}" fill="{color}" fill-opacity="0.18" stroke="{color}" stroke-width="2" vector-effect="non-scaling-stroke"/>"#
        )?;
    }
    for boundary_loop in polygon.loops() {
        let mut path = String::new();
        for (index, point) in boundary_loop.vertices.iter().enumerate() {
            let command = if index == 0 { 'M' } else { 'L' };
            write!(path, "{command} {} {} ", point.x, -i128::from(point.y))?;
        }
        writeln!(
            svg,
            r##"<path d="{path}Z" fill="none" stroke="#20262b" stroke-width="3" vector-effect="non-scaling-stroke"/>"##
        )?;
    }
    svg.push_str("</svg>\n");
    Ok(svg)
}

fn chord_line(
    svg: &mut String,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
    selected: bool,
) -> Result<(), std::fmt::Error> {
    let (color, width, dash) = if selected {
        ("#007f73", 4, "none")
    } else {
        ("#7b8790", 2, "5 4")
    };
    writeln!(
        svg,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="{width}" stroke-dasharray="{dash}"/>"#
    )
}

fn svg_x(coordinate: i64, scale: usize, margin: usize) -> Result<usize, FormatError> {
    usize::try_from(coordinate)
        .ok()
        .and_then(|value| value.checked_mul(scale))
        .and_then(|value| value.checked_add(margin))
        .ok_or(FormatError::DimensionOverflow)
}

fn svg_y(
    coordinate: i64,
    grid_height: usize,
    scale: usize,
    margin: usize,
) -> Result<usize, FormatError> {
    let coordinate = usize::try_from(coordinate).map_err(|_| FormatError::DimensionOverflow)?;
    grid_height
        .checked_sub(coordinate)
        .and_then(|value| value.checked_mul(scale))
        .and_then(|value| value.checked_add(margin))
        .ok_or(FormatError::DimensionOverflow)
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error(transparent)]
    Boundary(#[from] BoundaryError),
    #[error("SVG dimensions or coordinates overflowed usize")]
    DimensionOverflow,
    #[error("SVG chord selection vectors do not match their chord families")]
    OverlayDimensionMismatch,
    #[error("cannot render an empty polygon")]
    EmptyPolygon,
    #[error("formatting SVG failed")]
    Formatting(#[from] std::fmt::Error),
}
