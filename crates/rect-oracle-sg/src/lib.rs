//! Explicit Soltan--Gorpinevich oracle for ordinary grid-cell polygons.

use std::collections::{BTreeSet, HashSet, VecDeque};
use std::time::Instant;

use rect_core::{
    Boundary, BoundaryError, Certificate, Coord, Diagnostics, DissectionResult, ExactRatio,
    GeometryError, GridComponent, GridRect, HorizontalChord, HorizontalChordId, Point,
    ValidationError, VerticalChord, VerticalChordId, closed_chords_intersect, validate_dissection,
};
use rect_graph::{BipartiteGraph, Matching, VertexCover, hopcroft_karp, minimum_vertex_cover};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct SgAnalysis {
    pub boundary: Boundary,
    pub horizontal_chords: Vec<HorizontalChord>,
    pub vertical_chords: Vec<VerticalChord>,
    pub conflict_graph: BipartiteGraph,
    pub matching: Matching,
    pub vertex_cover: VertexCover,
    pub selected_horizontal: Vec<bool>,
    pub selected_vertical: Vec<bool>,
    pub optimum_rectangle_count: usize,
}

#[derive(Clone, Debug)]
pub struct SgGeometry {
    pub boundary: Boundary,
    pub horizontal_chords: Vec<HorizontalChord>,
    pub vertical_chords: Vec<VerticalChord>,
}

/// Extracts the supported boundary and effective chord families without
/// constructing the conflict graph or running a matching algorithm.
///
/// # Errors
///
/// Returns [`SgError`] when boundary extraction, topology validation, or
/// effective-chord enumeration fails.
pub fn analyze_geometry<C>(component: &GridComponent<C>) -> Result<SgGeometry, SgError> {
    let boundary = Boundary::from_component(component)?;
    if boundary.outer_loop_count() != 1 {
        return Err(SgError::UnsupportedBoundaryTopology {
            outer_loops: boundary.outer_loop_count(),
        });
    }
    let (horizontal_chords, vertical_chords) = enumerate_effective_chords(component, &boundary)?;
    Ok(SgGeometry {
        boundary,
        horizontal_chords,
        vertical_chords,
    })
}

/// Builds and verifies the complete explicit classical reduction.
///
/// # Errors
///
/// Returns [`SgError`] when a boundary, chord, graph, or formula invariant fails.
pub fn analyze<C>(component: &GridComponent<C>) -> Result<SgAnalysis, SgError> {
    let geometry = analyze_geometry(component)?;
    let boundary = geometry.boundary;
    let horizontal_chords = geometry.horizontal_chords;
    let vertical_chords = geometry.vertical_chords;
    let conflict_graph = build_conflict_graph(&horizontal_chords, &vertical_chords)?;
    let matching = hopcroft_karp(&conflict_graph);
    let vertex_cover = minimum_vertex_cover(&conflict_graph, &matching);
    let selected_horizontal = vertex_cover.left.iter().map(|selected| !selected).collect();
    let selected_vertical = vertex_cover
        .right
        .iter()
        .map(|selected| !selected)
        .collect();
    let chord_count = horizontal_chords.len() + vertical_chords.len();
    let independent_count = chord_count
        .checked_sub(matching.size)
        .ok_or(SgError::FormulaUnderflow)?;
    let base = boundary
        .reflex_vertices
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_sub(boundary.hole_count()))
        .ok_or(SgError::FormulaUnderflow)?;
    let optimum_rectangle_count = base
        .checked_sub(independent_count)
        .ok_or(SgError::FormulaUnderflow)?;

    let analysis = SgAnalysis {
        boundary,
        horizontal_chords,
        vertical_chords,
        conflict_graph,
        matching,
        vertex_cover,
        selected_horizontal,
        selected_vertical,
        optimum_rectangle_count,
    };
    validate_analysis(component, &analysis)?;
    Ok(analysis)
}

/// Solves a supported grid component through explicit SG matching and completion.
///
/// # Errors
///
/// Returns [`SgError`] when reduction, certificate, completion, or output validation fails.
pub fn solve<C>(component: &GridComponent<C>) -> Result<DissectionResult, SgError> {
    let started = Instant::now();
    let analysis = analyze(component)?;
    let analyzed_at = Instant::now();
    let rectangles = complete_dissection(component, &analysis)?;
    let completed_at = Instant::now();
    if rectangles.len() != analysis.optimum_rectangle_count {
        return Err(SgError::CompletionCount {
            expected: analysis.optimum_rectangle_count,
            actual: rectangles.len(),
        });
    }

    let selected_horizontal = analysis
        .selected_horizontal
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let selected_vertical = analysis
        .selected_vertical
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let matching_edges = analysis
        .matching
        .left_to_right
        .iter()
        .enumerate()
        .filter_map(|(left, right)| right.map(|right| (left, right)))
        .collect::<Vec<_>>();

    let result = DissectionResult {
        optimum_rectangle_count: analysis.optimum_rectangle_count,
        rectangles,
        diagnostics: Diagnostics {
            cell_count: component.cell_count(),
            boundary_complexity: analysis.boundary.boundary_complexity(),
            outer_loop_count: analysis.boundary.outer_loop_count(),
            hole_count: analysis.boundary.hole_count(),
            reflex_vertex_count: analysis.boundary.reflex_vertices.len(),
            horizontal_chord_count: analysis.horizontal_chords.len(),
            vertical_chord_count: analysis.vertical_chords.len(),
            total_chord_count: analysis.horizontal_chords.len() + analysis.vertical_chords.len(),
            explicit_conflict_edge_count: Some(analysis.conflict_graph.edge_count()),
            conflict_edge_density: ExactRatio::new(
                analysis.conflict_graph.edge_count() as u128,
                (analysis.horizontal_chords.len() as u128)
                    * (analysis.vertical_chords.len() as u128),
            ),
            maximum_matching_size: analysis.matching.size,
            minimum_vertex_cover_size: analysis.vertex_cover.size,
            output_rectangle_count: analysis.optimum_rectangle_count,
            phase_microseconds: [
                (
                    "boundary_chords_matching".to_owned(),
                    analyzed_at.duration_since(started).as_micros(),
                ),
                (
                    "geometric_completion".to_owned(),
                    completed_at.duration_since(analyzed_at).as_micros(),
                ),
            ]
            .into_iter()
            .collect(),
            ..Diagnostics::default()
        },
        certificate: Some(Certificate {
            kind: "soltan-gorpinevich-explicit".to_owned(),
            payload: json!({
                "horizontal_chords": analysis.horizontal_chords,
                "vertical_chords": analysis.vertical_chords,
                "matching_edges": matching_edges,
                "cover_left": analysis.vertex_cover.left,
                "cover_right": analysis.vertex_cover.right,
                "selected_horizontal": selected_horizontal,
                "selected_vertical": selected_vertical,
                "formula": {
                    "reflex_vertices": analysis.boundary.reflex_vertices.len(),
                    "holes": analysis.boundary.hole_count(),
                    "effective_chords": analysis.horizontal_chords.len() + analysis.vertical_chords.len(),
                    "matching": analysis.matching.size,
                    "rectangles": analysis.optimum_rectangle_count,
                }
            }),
        }),
    };
    validate_dissection(component, &result)?;
    Ok(result)
}

/// Enumerates Definition 7 effective chords for an ordinary grid-cell polygon.
///
/// # Errors
///
/// Returns [`SgError`] when exact coordinates cannot be converted or chord
/// construction fails.
pub fn enumerate_effective_chords<C>(
    component: &GridComponent<C>,
    boundary: &Boundary,
) -> Result<(Vec<HorizontalChord>, Vec<VerticalChord>), SgError> {
    let points = boundary
        .reflex_vertices
        .iter()
        .map(|vertex| vertex.point)
        .collect::<Vec<_>>();
    let mut horizontal_records = BTreeSet::new();
    let mut vertical_records = BTreeSet::new();
    for first_index in 0..points.len() {
        for second_index in (first_index + 1)..points.len() {
            let first = points[first_index];
            let second = points[second_index];
            if first.y == second.y {
                let left = first.x.min(second.x);
                let right = first.x.max(second.x);
                if horizontal_open_interval_is_interior(component, left, right, first.y)? {
                    horizontal_records.insert((first.y, left, right));
                }
            }
            if first.x == second.x {
                let bottom = first.y.min(second.y);
                let top = first.y.max(second.y);
                if vertical_open_interval_is_interior(component, first.x, bottom, top)? {
                    vertical_records.insert((first.x, bottom, top));
                }
            }
        }
    }

    let horizontal_chords = horizontal_records
        .into_iter()
        .enumerate()
        .map(|(index, (y, left, right))| {
            HorizontalChord::new(HorizontalChordId(index), left, right, y)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let vertical_chords = vertical_records
        .into_iter()
        .enumerate()
        .map(|(index, (x, bottom, top))| VerticalChord::new(VerticalChordId(index), x, bottom, top))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((horizontal_chords, vertical_chords))
}

fn horizontal_open_interval_is_interior<C>(
    component: &GridComponent<C>,
    left: Coord,
    right: Coord,
    y: Coord,
) -> Result<bool, SgError> {
    let left = coordinate_to_usize(left)?;
    let right = coordinate_to_usize(right)?;
    let y = coordinate_to_usize(y)?;
    if y == 0 {
        return Ok(false);
    }
    Ok((left..right).all(|x| component.contains_cell(x, y - 1) && component.contains_cell(x, y)))
}

fn vertical_open_interval_is_interior<C>(
    component: &GridComponent<C>,
    x: Coord,
    bottom: Coord,
    top: Coord,
) -> Result<bool, SgError> {
    let x = coordinate_to_usize(x)?;
    let bottom = coordinate_to_usize(bottom)?;
    let top = coordinate_to_usize(top)?;
    if x == 0 {
        return Ok(false);
    }
    Ok((bottom..top).all(|y| component.contains_cell(x - 1, y) && component.contains_cell(x, y)))
}

fn coordinate_to_usize(value: Coord) -> Result<usize, SgError> {
    usize::try_from(value).map_err(|_| SgError::CoordinateConversion { value })
}

/// Builds all closed horizontal--vertical chord intersections explicitly.
///
/// # Errors
///
/// Returns [`SgError`] if an endpoint cannot be inserted into declared graph dimensions.
pub fn build_conflict_graph(
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
) -> Result<BipartiteGraph, SgError> {
    let mut graph = BipartiteGraph::new(horizontal_chords.len(), vertical_chords.len());
    for (left, &horizontal) in horizontal_chords.iter().enumerate() {
        for (right, &vertical) in vertical_chords.iter().enumerate() {
            if closed_chords_intersect(horizontal, vertical) {
                graph.add_edge(left, right)?;
            }
        }
    }
    Ok(graph)
}

/// Checks chord validity, cover completeness, independence, and Konig equality.
///
/// # Errors
///
/// Returns [`SgError`] on the first invalid certificate or geometric object.
pub fn validate_analysis<C>(
    component: &GridComponent<C>,
    analysis: &SgAnalysis,
) -> Result<(), SgError> {
    let reflex_points = analysis
        .boundary
        .reflex_vertices
        .iter()
        .map(|vertex| vertex.point)
        .collect::<HashSet<_>>();
    for &chord in &analysis.horizontal_chords {
        if !reflex_points.contains(&Point::new(chord.left(), chord.y()))
            || !reflex_points.contains(&Point::new(chord.right(), chord.y()))
            || !horizontal_open_interval_is_interior(
                component,
                chord.left(),
                chord.right(),
                chord.y(),
            )?
        {
            return Err(SgError::InvalidEffectiveChord);
        }
    }
    for &chord in &analysis.vertical_chords {
        if !reflex_points.contains(&Point::new(chord.x(), chord.bottom()))
            || !reflex_points.contains(&Point::new(chord.x(), chord.top()))
            || !vertical_open_interval_is_interior(
                component,
                chord.x(),
                chord.bottom(),
                chord.top(),
            )?
        {
            return Err(SgError::InvalidEffectiveChord);
        }
    }
    for (left, right) in analysis.conflict_graph.edges() {
        if !analysis.vertex_cover.left[left] && !analysis.vertex_cover.right[right] {
            return Err(SgError::UncoveredConflictEdge { left, right });
        }
        if analysis.selected_horizontal[left] && analysis.selected_vertical[right] {
            return Err(SgError::NonIndependentSelection { left, right });
        }
    }
    if analysis.matching.size != analysis.vertex_cover.size {
        return Err(SgError::MatchingCoverMismatch {
            matching: analysis.matching.size,
            cover: analysis.vertex_cover.size,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
struct HorizontalUnitCut {
    x: usize,
    y: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
struct VerticalUnitCut {
    x: usize,
    y: usize,
}

#[derive(Clone, Debug, Default)]
struct Cuts {
    horizontal: BTreeSet<HorizontalUnitCut>,
    vertical: BTreeSet<VerticalUnitCut>,
}

impl Cuts {
    fn from_selection(
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<Self, SgError> {
        let mut cuts = Self::default();
        for (index, &selected) in selected_horizontal.iter().enumerate() {
            if selected {
                let chord = horizontal_chords[index];
                let left = coordinate_to_usize(chord.left())?;
                let right = coordinate_to_usize(chord.right())?;
                let y = coordinate_to_usize(chord.y())?;
                cuts.horizontal
                    .extend((left..right).map(|x| HorizontalUnitCut { x, y }));
            }
        }
        for (index, &selected) in selected_vertical.iter().enumerate() {
            if selected {
                let chord = vertical_chords[index];
                let x = coordinate_to_usize(chord.x())?;
                let bottom = coordinate_to_usize(chord.bottom())?;
                let top = coordinate_to_usize(chord.top())?;
                cuts.vertical
                    .extend((bottom..top).map(|y| VerticalUnitCut { x, y }));
            }
        }
        Ok(cuts)
    }
}

#[derive(Clone, Copy, Debug)]
enum Direction {
    East,
    North,
    West,
    South,
}

impl Direction {
    const fn is_horizontal(self) -> bool {
        matches!(self, Self::East | Self::West)
    }
}

fn complete_dissection<C>(
    component: &GridComponent<C>,
    analysis: &SgAnalysis,
) -> Result<Vec<GridRect>, SgError> {
    complete_with_selected_chords(
        component,
        analysis,
        &analysis.selected_horizontal,
        &analysis.selected_vertical,
    )
}

/// Performs the classical horizontal-then-vertical simple-chord completion.
///
/// # Errors
///
/// Returns [`SgError`] for dimension mismatches, invalid simple chords,
/// nontermination, or nonrectangular final regions.
pub fn complete_with_selected_chords<C>(
    component: &GridComponent<C>,
    analysis: &SgAnalysis,
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
) -> Result<Vec<GridRect>, SgError> {
    complete_with_chord_families(
        component,
        &analysis.horizontal_chords,
        &analysis.vertical_chords,
        selected_horizontal,
        selected_vertical,
    )
}

/// Performs geometric completion from chord families without requiring an
/// explicit conflict graph or matching object.
///
/// # Errors
///
/// Returns [`SgError`] for dimension mismatches, invalid simple chords,
/// nontermination, or nonrectangular final regions.
pub fn complete_with_chord_families<C>(
    component: &GridComponent<C>,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
) -> Result<Vec<GridRect>, SgError> {
    if selected_horizontal.len() != horizontal_chords.len()
        || selected_vertical.len() != vertical_chords.len()
    {
        return Err(SgError::SelectionLengthMismatch);
    }
    let mut cuts = Cuts::from_selection(
        horizontal_chords,
        vertical_chords,
        selected_horizontal,
        selected_vertical,
    )?;
    complete_axis(component, &mut cuts, true)?;
    complete_axis(component, &mut cuts, false)?;
    rectangles_from_cuts(component, &cuts)
}

fn complete_axis<C>(
    component: &GridComponent<C>,
    cuts: &mut Cuts,
    horizontal: bool,
) -> Result<(), SgError> {
    let horizontal_height = component
        .grid_height
        .checked_add(1)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let vertical_width = component
        .grid_width
        .checked_add(1)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let horizontal_slots = component
        .grid_width
        .checked_mul(horizontal_height)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let vertical_slots = vertical_width
        .checked_mul(component.grid_height)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let maximum_unit_cuts = horizontal_slots
        .checked_add(vertical_slots)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    for _ in 0..=maximum_unit_cuts {
        let Some((point, direction)) = find_concave_ray(component, cuts, horizontal) else {
            return Ok(());
        };
        let added = extend_simple_chord(component, cuts, point, direction);
        if added == 0 {
            return Err(SgError::InvalidSimpleChord { point });
        }
    }
    Err(SgError::CompletionDidNotTerminate)
}

fn find_concave_ray<C>(
    component: &GridComponent<C>,
    cuts: &Cuts,
    horizontal: bool,
) -> Option<((usize, usize), Direction)> {
    for y in 0..=component.grid_height {
        for x in 0..=component.grid_width {
            let inside = local_quadrants(component, x, y);
            let blocked = local_blocked_rays(cuts, inside, x, y);
            if !blocked.iter().any(|&value| value) {
                continue;
            }
            let (roots, sizes) = local_angle_components(inside, blocked);
            for (direction, first, second) in [
                (Direction::East, 1, 2),
                (Direction::North, 2, 3),
                (Direction::West, 3, 0),
                (Direction::South, 0, 1),
            ] {
                if direction.is_horizontal() != horizontal {
                    continue;
                }
                let ray_index = match direction {
                    Direction::East => 0,
                    Direction::North => 1,
                    Direction::West => 2,
                    Direction::South => 3,
                };
                if inside[first]
                    && inside[second]
                    && !blocked[ray_index]
                    && roots[first] == roots[second]
                    && sizes[roots[first]] >= 3
                {
                    return Some(((x, y), direction));
                }
            }
        }
    }
    None
}

fn local_quadrants<C>(component: &GridComponent<C>, x: usize, y: usize) -> [bool; 4] {
    [
        x > 0 && y > 0 && component.contains_cell(x - 1, y - 1),
        y > 0 && component.contains_cell(x, y - 1),
        component.contains_cell(x, y),
        x > 0 && component.contains_cell(x - 1, y),
    ]
}

fn local_blocked_rays(cuts: &Cuts, inside: [bool; 4], x: usize, y: usize) -> [bool; 4] {
    [
        cuts.horizontal.contains(&HorizontalUnitCut { x, y }) || inside[1] != inside[2],
        cuts.vertical.contains(&VerticalUnitCut { x, y }) || inside[2] != inside[3],
        (x > 0 && cuts.horizontal.contains(&HorizontalUnitCut { x: x - 1, y }))
            || inside[3] != inside[0],
        (y > 0 && cuts.vertical.contains(&VerticalUnitCut { x, y: y - 1 }))
            || inside[0] != inside[1],
    ]
}

fn local_angle_components(inside: [bool; 4], blocked: [bool; 4]) -> ([usize; 4], [usize; 4]) {
    let mut roots = [0, 1, 2, 3];
    for (ray, first, second) in [(0, 1, 2), (1, 2, 3), (2, 3, 0), (3, 0, 1)] {
        if inside[first] && inside[second] && !blocked[ray] {
            union_roots(&mut roots, first, second);
        }
    }
    for index in 0..4 {
        roots[index] = find_root(&roots, index);
    }
    let mut sizes = [0; 4];
    for index in 0..4 {
        if inside[index] {
            sizes[roots[index]] += 1;
        }
    }
    (roots, sizes)
}

fn find_root(roots: &[usize; 4], mut index: usize) -> usize {
    while roots[index] != index {
        index = roots[index];
    }
    index
}

fn union_roots(roots: &mut [usize; 4], first: usize, second: usize) {
    let first_root = find_root(roots, first);
    let second_root = find_root(roots, second);
    if first_root != second_root {
        roots[second_root] = first_root;
    }
}

fn extend_simple_chord<C>(
    component: &GridComponent<C>,
    cuts: &mut Cuts,
    point: (usize, usize),
    direction: Direction,
) -> usize {
    let mut horizontal_additions = Vec::new();
    let mut vertical_additions = Vec::new();
    let (mut x, mut y) = point;
    loop {
        let next = match direction {
            Direction::East => {
                if y == 0
                    || !component.contains_cell(x, y - 1)
                    || !component.contains_cell(x, y)
                    || cuts.horizontal.contains(&HorizontalUnitCut { x, y })
                {
                    break;
                }
                horizontal_additions.push(HorizontalUnitCut { x, y });
                x += 1;
                (x, y)
            }
            Direction::West => {
                if x == 0 || y == 0 {
                    break;
                }
                let unit_x = x - 1;
                if !component.contains_cell(unit_x, y - 1)
                    || !component.contains_cell(unit_x, y)
                    || cuts
                        .horizontal
                        .contains(&HorizontalUnitCut { x: unit_x, y })
                {
                    break;
                }
                horizontal_additions.push(HorizontalUnitCut { x: unit_x, y });
                x -= 1;
                (x, y)
            }
            Direction::North => {
                if x == 0
                    || !component.contains_cell(x - 1, y)
                    || !component.contains_cell(x, y)
                    || cuts.vertical.contains(&VerticalUnitCut { x, y })
                {
                    break;
                }
                vertical_additions.push(VerticalUnitCut { x, y });
                y += 1;
                (x, y)
            }
            Direction::South => {
                if x == 0 || y == 0 {
                    break;
                }
                let unit_y = y - 1;
                if !component.contains_cell(x - 1, unit_y)
                    || !component.contains_cell(x, unit_y)
                    || cuts.vertical.contains(&VerticalUnitCut { x, y: unit_y })
                {
                    break;
                }
                vertical_additions.push(VerticalUnitCut { x, y: unit_y });
                y -= 1;
                (x, y)
            }
        };
        if perpendicular_boundary_at(component, cuts, next, direction.is_horizontal()) {
            break;
        }
    }
    let added = horizontal_additions.len() + vertical_additions.len();
    cuts.horizontal.extend(horizontal_additions);
    cuts.vertical.extend(vertical_additions);
    added
}

fn perpendicular_boundary_at<C>(
    component: &GridComponent<C>,
    cuts: &Cuts,
    point: (usize, usize),
    horizontal_chord: bool,
) -> bool {
    let (x, y) = point;
    let inside = local_quadrants(component, x, y);
    if horizontal_chord {
        cuts.vertical.contains(&VerticalUnitCut { x, y })
            || (y > 0 && cuts.vertical.contains(&VerticalUnitCut { x, y: y - 1 }))
            || inside[2] != inside[3]
            || inside[0] != inside[1]
    } else {
        cuts.horizontal.contains(&HorizontalUnitCut { x, y })
            || (x > 0 && cuts.horizontal.contains(&HorizontalUnitCut { x: x - 1, y }))
            || inside[1] != inside[2]
            || inside[3] != inside[0]
    }
}

fn rectangles_from_cuts<C>(
    component: &GridComponent<C>,
    cuts: &Cuts,
) -> Result<Vec<GridRect>, SgError> {
    let cell_set = component.cells.iter().copied().collect::<HashSet<_>>();
    let mut unseen = cell_set.clone();
    let mut rectangles = Vec::new();
    while let Some(&seed) = unseen.iter().next() {
        unseen.remove(&seed);
        let mut queue = VecDeque::from([seed]);
        let mut region = vec![seed];
        while let Some(cell) = queue.pop_front() {
            for neighbor in uncut_neighbors(cell, component, cuts) {
                if unseen.remove(&neighbor) {
                    queue.push_back(neighbor);
                    region.push(neighbor);
                }
            }
        }
        let x0 = region.iter().map(|cell| cell.x).min().unwrap();
        let y0 = region.iter().map(|cell| cell.y).min().unwrap();
        let x1 = region.iter().map(|cell| cell.x + 1).max().unwrap();
        let y1 = region.iter().map(|cell| cell.y + 1).max().unwrap();
        let rectangle = GridRect::new(x0, y0, x1, y1)?;
        if rectangle.area() != region.len()
            || !(y0..y1).all(|y| (x0..x1).all(|x| cell_set.contains(&rect_core::Cell { x, y })))
        {
            return Err(SgError::NonRectangularCompletionRegion {
                seed_x: seed.x,
                seed_y: seed.y,
            });
        }
        rectangles.push(rectangle);
    }
    rectangles.sort_unstable();
    Ok(rectangles)
}

fn uncut_neighbors<C>(
    cell: rect_core::Cell,
    component: &GridComponent<C>,
    cuts: &Cuts,
) -> impl Iterator<Item = rect_core::Cell> {
    let mut neighbors = Vec::with_capacity(4);
    if cell.x > 0
        && component.contains_cell(cell.x - 1, cell.y)
        && !cuts.vertical.contains(&VerticalUnitCut {
            x: cell.x,
            y: cell.y,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x - 1,
            y: cell.y,
        });
    }
    if component.contains_cell(cell.x + 1, cell.y)
        && !cuts.vertical.contains(&VerticalUnitCut {
            x: cell.x + 1,
            y: cell.y,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x + 1,
            y: cell.y,
        });
    }
    if cell.y > 0
        && component.contains_cell(cell.x, cell.y - 1)
        && !cuts.horizontal.contains(&HorizontalUnitCut {
            x: cell.x,
            y: cell.y,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x,
            y: cell.y - 1,
        });
    }
    if component.contains_cell(cell.x, cell.y + 1)
        && !cuts.horizontal.contains(&HorizontalUnitCut {
            x: cell.x,
            y: cell.y + 1,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x,
            y: cell.y + 1,
        });
    }
    neighbors.into_iter()
}

#[derive(Debug, Error)]
pub enum SgError {
    #[error(transparent)]
    Boundary(#[from] BoundaryError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Graph(#[from] rect_graph::GraphError),
    #[error("solver produced an invalid dissection: {0}")]
    InvalidOutput(#[from] ValidationError),
    #[error("grid coordinate {value} cannot be represented as usize")]
    CoordinateConversion { value: Coord },
    #[error("ordinary grid component unexpectedly has {outer_loops} outer loops")]
    UnsupportedBoundaryTopology { outer_loops: usize },
    #[error("rectangle-count formula underflowed; geometric invariants are inconsistent")]
    FormulaUnderflow,
    #[error("an enumerated effective chord violates Definition 7 in the grid model")]
    InvalidEffectiveChord,
    #[error("minimum vertex cover does not cover conflict edge ({left}, {right})")]
    UncoveredConflictEdge { left: usize, right: usize },
    #[error("selected chord family contains conflicting pair ({left}, {right})")]
    NonIndependentSelection { left: usize, right: usize },
    #[error("matching size {matching} differs from minimum vertex-cover size {cover}")]
    MatchingCoverMismatch { matching: usize, cover: usize },
    #[error("geometric completion could not extend a simple chord from {point:?}")]
    InvalidSimpleChord { point: (usize, usize) },
    #[error("geometric completion did not terminate within the finite unit-cut bound")]
    CompletionDidNotTerminate,
    #[error("completion region containing ({seed_x}, {seed_y}) is not a rectangle")]
    NonRectangularCompletionRegion { seed_x: usize, seed_y: usize },
    #[error("completion produced {actual} rectangles, formula requires {expected}")]
    CompletionCount { expected: usize, actual: usize },
    #[error("selected-chord bit vectors do not match the chord-family dimensions")]
    SelectionLengthMismatch,
}

#[cfg(test)]
mod tests {
    use rect_core::{ColorGrid, GridComponent, validate_dissection};
    use rect_oracle_exact_cover as exact_cover;

    use super::solve;

    fn foreground_component(width: usize, height: usize, cells: Vec<bool>) -> GridComponent<bool> {
        ColorGrid::new(width, height, cells)
            .unwrap()
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
            .max_by_key(GridComponent::cell_count)
            .unwrap()
    }

    #[test]
    fn agrees_on_l_tromino_plus_and_ring() {
        let cases = [
            (2, 2, vec![true, true, true, false]),
            (
                3,
                3,
                vec![false, true, false, true, true, true, false, true, false],
            ),
            (
                3,
                3,
                vec![true, true, true, true, false, true, true, true, true],
            ),
        ];
        for (width, height, cells) in cases {
            let component = foreground_component(width, height, cells);
            let expected = exact_cover::solve(&component).unwrap();
            let actual = solve(&component).unwrap();
            assert_eq!(
                actual.optimum_rectangle_count,
                expected.optimum_rectangle_count
            );
            validate_dissection(&component, &actual).unwrap();
        }
    }

    #[test]
    fn exhaustively_agrees_through_three_by_three() {
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            let grid = ColorGrid::new(3, 3, cells).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let expected = exact_cover::solve(&component).unwrap();
                let actual = solve(&component).unwrap_or_else(|error| {
                    panic!("mask {mask:#05x}, component {:?}: {error}", component.cells)
                });
                assert_eq!(
                    actual.optimum_rectangle_count, expected.optimum_rectangle_count,
                    "mask {mask:#05x}, component {:?}",
                    component.cells
                );
            }
        }
    }
}
