use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rect_core::{
    ColorGrid, GridComponent, HorizontalChord, HorizontalChordId, VerticalChord, VerticalChordId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdversarialInstance {
    pub name: String,
    pub family: String,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    pub parameters: BTreeMap<String, usize>,
}

/// Returns an integer-grid realization of the clean complete-bipartite family.
///
/// The construction uses disjoint one-cell notch intervals in a rectangular
/// background.  The horizontal notch pairs produce two horizontal chords per
/// interval, and the vertical notch pairs produce two vertical chords per
/// interval.  All four families are separated by a wide integer margin, so
/// the only cross-orientation intersections are the intended complete
/// bipartite ones.
///
/// # Errors
///
///
/// # Errors
///
/// Returns [`CleanCompleteBipartiteError`] when `t` is zero or the generated
/// dimensions overflow `usize`.
pub fn clean_complete_bipartite_grid(
    t: usize,
) -> Result<AdversarialInstance, CleanCompleteBipartiteError> {
    if t == 0 {
        return Err(CleanCompleteBipartiteError::ZeroParameter);
    }
    let margin = t
        .checked_mul(2)
        .and_then(|value| value.checked_add(6))
        .ok_or(CleanCompleteBipartiteError::DimensionOverflow)?;
    let span = t
        .checked_mul(3)
        .and_then(|value| value.checked_add(2))
        .ok_or(CleanCompleteBipartiteError::DimensionOverflow)?;
    let width = margin
        .checked_mul(2)
        .and_then(|value| value.checked_add(span))
        .ok_or(CleanCompleteBipartiteError::DimensionOverflow)?;
    let height = width;
    let cell_count = width
        .checked_mul(height)
        .ok_or(CleanCompleteBipartiteError::DimensionOverflow)?;
    let mut cells = vec![true; cell_count];

    for index in 0..t {
        let interval_start = margin
            + index
                .checked_mul(3)
                .ok_or(CleanCompleteBipartiteError::DimensionOverflow)?;
        let left_depth = 2 + index;
        let right_depth = 2 + t + index;
        for y in interval_start..=interval_start {
            for x in 0..left_depth {
                set_cell(&mut cells, width, x, y, false);
            }
            for x in width - right_depth..width {
                set_cell(&mut cells, width, x, y, false);
            }
        }
    }

    for index in 0..t {
        let interval_start = margin
            + index
                .checked_mul(3)
                .ok_or(CleanCompleteBipartiteError::DimensionOverflow)?;
        let bottom_depth = 2 + index;
        let top_depth = 2 + t + index;
        for x in interval_start..=interval_start {
            for y in 0..bottom_depth {
                set_cell(&mut cells, width, x, y, false);
            }
            for y in height - top_depth..height {
                set_cell(&mut cells, width, x, y, false);
            }
        }
    }

    Ok(AdversarialInstance {
        name: format!("clean-complete-bipartite-t{t}"),
        family: "clean-complete-bipartite".to_owned(),
        width,
        height,
        cells,
        parameters: [("t".to_owned(), t)].into_iter().collect(),
    })
}

#[derive(Debug, Error)]
pub enum CleanCompleteBipartiteError {
    #[error("complete-bipartite parameter t must be positive")]
    ZeroParameter,
    #[error("complete-bipartite dimensions overflow usize")]
    DimensionOverflow,
}

impl AdversarialInstance {
    /// Converts the fixture to the common exact JSON-grid model.
    ///
    /// # Errors
    ///
    /// Returns [`AdversarialError`] if dimensions and cell count disagree.
    pub fn grid(&self) -> Result<ColorGrid<bool>, AdversarialError> {
        ColorGrid::new(self.width, self.height, self.cells.clone())
            .map_err(|error| AdversarialError::InvalidGrid(error.to_string()))
    }

    /// Returns all foreground components in deterministic component order.
    ///
    /// # Errors
    ///
    /// Returns [`AdversarialError`] if the fixture is malformed.
    pub fn foreground_components(&self) -> Result<Vec<GridComponent<bool>>, AdversarialError> {
        Ok(self
            .grid()?
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
            .collect())
    }

    /// Writes the same JSON schema accepted by `rect-cli` plus fixture metadata.
    ///
    /// # Errors
    ///
    /// Returns [`AdversarialError`] for serialization or filesystem failures.
    pub fn write_json(&self, path: &Path) -> Result<(), AdversarialError> {
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointContactKind {
    VerticalAtHorizontalLeft,
    VerticalAtHorizontalRight,
    HorizontalAtVerticalBottom,
    HorizontalAtVerticalTop,
    BothEndpoints,
    TJunction,
    StrictInterior,
    SameCoordinateNonIntersecting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EndpointChordCase {
    pub kind: EndpointContactKind,
    pub horizontal: HorizontalChord,
    pub vertical: VerticalChord,
    pub expected_intersection: bool,
}

#[must_use]
/// Returns the complete endpoint-contact relation matrix used by regression tests.
///
/// # Panics
///
/// Panics only if a compile-time chord specification is accidentally made degenerate.
pub fn endpoint_chord_cases() -> Vec<EndpointChordCase> {
    let specifications = [
        (
            EndpointContactKind::VerticalAtHorizontalLeft,
            (0, 4, 2),
            (0, 0, 4),
            true,
        ),
        (
            EndpointContactKind::VerticalAtHorizontalRight,
            (0, 4, 2),
            (4, 0, 4),
            true,
        ),
        (
            EndpointContactKind::HorizontalAtVerticalBottom,
            (0, 4, 0),
            (2, 0, 4),
            true,
        ),
        (
            EndpointContactKind::HorizontalAtVerticalTop,
            (0, 4, 4),
            (2, 0, 4),
            true,
        ),
        (
            EndpointContactKind::BothEndpoints,
            (0, 4, 0),
            (0, 0, 4),
            true,
        ),
        (EndpointContactKind::TJunction, (0, 4, 2), (2, 2, 5), true),
        (
            EndpointContactKind::StrictInterior,
            (0, 4, 2),
            (2, 0, 4),
            true,
        ),
        (
            EndpointContactKind::SameCoordinateNonIntersecting,
            (0, 1, 2),
            (0, 3, 5),
            false,
        ),
    ];
    specifications
        .into_iter()
        .enumerate()
        .map(
            |(index, (kind, (left, right, y), (x, bottom, top), expected_intersection))| {
                EndpointChordCase {
                    kind,
                    horizontal: HorizontalChord::new(HorizontalChordId(index), left, right, y)
                        .expect("static horizontal chord is valid"),
                    vertical: VerticalChord::new(VerticalChordId(index), x, bottom, top)
                        .expect("static vertical chord is valid"),
                    expected_intersection,
                }
            },
        )
        .collect()
}

#[must_use]
pub fn endpoint_contact_instances() -> Vec<AdversarialInstance> {
    let dense = dense_conflict_grid(2, 2);
    let mut near = dense.clone();
    "endpoint-near-identical-minus-one-cell".clone_into(&mut near.name);
    "endpoint-contact".clone_into(&mut near.family);
    if let Some(index) = near.cells.iter().rposition(|&cell| cell) {
        near.cells[index] = false;
    }
    vec![
        AdversarialInstance {
            name: "endpoint-dense-tabs".to_owned(),
            family: "endpoint-contact".to_owned(),
            ..dense
        },
        near,
    ]
}

#[must_use]
pub fn external_oracle_adversarial_instances() -> Vec<AdversarialInstance> {
    [
        one_hole_ring(5, 5),
        narrow_corridor(7, 5),
        comb(3, 4),
        double_comb(3, 5),
        staircase(5),
        orthogonal_spiral(7),
        dense_conflict_grid(2, 2),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, mut instance)| {
        instance.name = format!("cp-small-{index:02}-{}", instance.name);
        "external-oracle-adversarial".clone_into(&mut instance.family);
        instance
    })
    .collect()
}

#[must_use]
pub fn dense_conflict_grid(min_horizontal: usize, min_vertical: usize) -> AdversarialInstance {
    let horizontal_tabs = min_horizontal.div_ceil(2).max(1);
    let vertical_tabs = min_vertical.div_ceil(2).max(1);
    let inner_width = vertical_tabs * 2 + 3;
    let inner_height = horizontal_tabs * 2 + 3;
    let width = inner_width + 2;
    let height = inner_height + 2;
    let mut cells = vec![false; width * height];

    fill_rectangle(&mut cells, width, 1, 1, width - 1, height - 1);
    for tab in 0..horizontal_tabs {
        let y = 2 + tab * 2;
        set_cell(&mut cells, width, 0, y, true);
        set_cell(&mut cells, width, width - 1, y, true);
    }
    for tab in 0..vertical_tabs {
        let x = 2 + tab * 2;
        set_cell(&mut cells, width, x, 0, true);
        set_cell(&mut cells, width, x, height - 1, true);
    }

    AdversarialInstance {
        name: format!("dense-conflict-{min_horizontal}x{min_vertical}"),
        family: "dense-conflict".to_owned(),
        width,
        height,
        cells,
        parameters: [
            ("min_horizontal".to_owned(), min_horizontal),
            ("min_vertical".to_owned(), min_vertical),
        ]
        .into_iter()
        .collect(),
    }
}

#[must_use]
pub fn contains_complete_bipartite(
    graph: &rect_graph::BipartiteGraph,
    left_count: usize,
    right_count: usize,
) -> bool {
    if left_count == 0 {
        return graph.right_size() >= right_count;
    }
    if left_count > graph.left_size() || right_count > graph.right_size() {
        return false;
    }
    let common = vec![true; graph.right_size()];
    choose_left_vertices(graph, 0, left_count, right_count, &common)
}

fn choose_left_vertices(
    graph: &rect_graph::BipartiteGraph,
    start: usize,
    remaining: usize,
    right_count: usize,
    common: &[bool],
) -> bool {
    if remaining == 0 {
        return common.iter().filter(|&&present| present).count() >= right_count;
    }
    if graph.left_size() - start < remaining {
        return false;
    }
    for left in start..=graph.left_size() - remaining {
        let mut next_common = vec![false; graph.right_size()];
        for &right in graph.neighbors(left) {
            next_common[right] = common[right];
        }
        if next_common.iter().filter(|&&present| present).count() >= right_count
            && choose_left_vertices(graph, left + 1, remaining - 1, right_count, &next_common)
        {
            return true;
        }
    }
    false
}

#[must_use]
pub fn topological_stress_instances() -> Vec<AdversarialInstance> {
    vec![
        one_hole_ring(7, 7),
        multiple_holes(),
        nested_looking(),
        narrow_corridor(11, 7),
        comb(6, 5),
        double_comb(6, 7),
        staircase(8),
        orthogonal_spiral(11),
        many_reflex_few_chords(8),
        dense_conflict_grid(6, 2),
        long_collinear_runs(16, 7),
        disconnected_same_color(),
        diagonal_touch(),
    ]
}

#[must_use]
/// Builds a one-cell-thick ordinary ring.
///
/// # Panics
///
/// Panics when either dimension is smaller than three.
pub fn one_hole_ring(width: usize, height: usize) -> AdversarialInstance {
    assert!(width >= 3 && height >= 3);
    let cells = (0..width * height)
        .map(|index| {
            let x = index % width;
            let y = index / width;
            x == 0 || y == 0 || x + 1 == width || y + 1 == height
        })
        .collect();
    instance(
        "one-hole-ring",
        "topology",
        width,
        height,
        cells,
        [("holes".to_owned(), 1)].into_iter().collect(),
    )
}

fn multiple_holes() -> AdversarialInstance {
    let width = 11;
    let height = 7;
    let mut cells = vec![true; width * height];
    for (x0, y0, x1, y1) in [(2, 2, 4, 5), (7, 2, 9, 5)] {
        fill_rectangle(&mut cells, width, x0, y0, x1, y1);
        for y in y0..y1 {
            for x in x0..x1 {
                set_cell(&mut cells, width, x, y, false);
            }
        }
    }
    instance(
        "multiple-separated-holes",
        "topology",
        width,
        height,
        cells,
        [("holes".to_owned(), 2)].into_iter().collect(),
    )
}

fn nested_looking() -> AdversarialInstance {
    let width = 13;
    let height = 9;
    let mut cells = vec![true; width * height];
    fill_rectangle(&mut cells, width, 2, 2, 11, 7);
    for y in 2..7 {
        for x in 2..11 {
            set_cell(&mut cells, width, x, y, false);
        }
    }
    fill_rectangle(&mut cells, width, 5, 3, 8, 6);
    fill_rectangle(&mut cells, width, 6, 1, 7, 4);
    instance(
        "nested-looking-bridged-rings",
        "topology",
        width,
        height,
        cells,
        BTreeMap::new(),
    )
}

#[must_use]
pub fn narrow_corridor(width: usize, height: usize) -> AdversarialInstance {
    let mut cells = vec![false; width * height];
    fill_rectangle(&mut cells, width, 0, 0, width, 1);
    fill_rectangle(&mut cells, width, width - 1, 0, width, height);
    fill_rectangle(&mut cells, width, 2, height - 1, width, height);
    fill_rectangle(&mut cells, width, 2, 2, 3, height);
    instance(
        "one-cell-wide-corridor",
        "topology",
        width,
        height,
        cells,
        [("corridor_width".to_owned(), 1)].into_iter().collect(),
    )
}

#[must_use]
pub fn comb(teeth: usize, tooth_height: usize) -> AdversarialInstance {
    let width = teeth * 2 + 1;
    let height = tooth_height + 1;
    let mut cells = vec![false; width * height];
    fill_rectangle(&mut cells, width, 0, 0, width, 1);
    for tooth in 0..teeth {
        let x = tooth * 2;
        fill_rectangle(&mut cells, width, x, 1, x + 1, height);
    }
    instance(
        "comb",
        "topology",
        width,
        height,
        cells,
        [("teeth".to_owned(), teeth)].into_iter().collect(),
    )
}

#[must_use]
pub fn double_comb(teeth: usize, height: usize) -> AdversarialInstance {
    let width = teeth * 2 + 1;
    let mut cells = vec![false; width * height];
    fill_rectangle(&mut cells, width, 0, 0, width, 1);
    fill_rectangle(&mut cells, width, 0, height - 1, width, height);
    for tooth in 0..teeth {
        let lower_x = tooth * 2;
        fill_rectangle(&mut cells, width, lower_x, 1, lower_x + 1, height - 2);
        let upper_x = lower_x + 1;
        fill_rectangle(&mut cells, width, upper_x, 2, upper_x + 1, height - 1);
    }
    instance(
        "double-comb",
        "topology",
        width,
        height,
        cells,
        [("teeth_per_side".to_owned(), teeth)].into_iter().collect(),
    )
}

#[must_use]
pub fn staircase(steps: usize) -> AdversarialInstance {
    let mut cells = vec![false; steps * steps];
    for y in 0..steps {
        fill_rectangle(&mut cells, steps, 0, y, steps - y, y + 1);
    }
    instance(
        "staircase",
        "topology",
        steps,
        steps,
        cells,
        [("steps".to_owned(), steps)].into_iter().collect(),
    )
}

/// Builds a one-cell-wide orthogonal spiral.
///
/// # Panics
///
/// Panics unless `size` is odd and at least five.
#[must_use]
pub fn orthogonal_spiral(size: usize) -> AdversarialInstance {
    assert!(size >= 5 && size % 2 == 1);
    let mut cells = vec![false; size * size];
    let (mut left, mut right, mut bottom, mut top) = (0, size - 1, 0, size - 1);
    fill_horizontal(&mut cells, size, left, right, bottom);
    fill_vertical(&mut cells, size, right, bottom, top);
    fill_horizontal(&mut cells, size, left, right, top);
    while right >= left + 4 && top >= bottom + 4 {
        bottom += 2;
        fill_vertical(&mut cells, size, left, bottom, top);
        right -= 2;
        fill_horizontal(&mut cells, size, left, right, bottom);
        top -= 2;
        fill_vertical(&mut cells, size, right, bottom, top);
        left += 2;
        fill_horizontal(&mut cells, size, left, right, top);
    }
    instance(
        "orthogonal-spiral",
        "topology",
        size,
        size,
        cells,
        [("size".to_owned(), size)].into_iter().collect(),
    )
}

#[must_use]
pub fn alternating_notch_corridor(notches: usize) -> AdversarialInstance {
    let width = notches * 2 + 1;
    let height = 5;
    let mut cells = vec![true; width * height];
    for notch in 0..notches {
        let x = notch * 2 + 1;
        let range = if notch % 2 == 0 { 0..2 } else { 3..5 };
        for y in range {
            set_cell(&mut cells, width, x, y, false);
        }
    }
    instance(
        "alternating-notch-corridor",
        "completion-heavy",
        width,
        height,
        cells,
        [("notches".to_owned(), notches)].into_iter().collect(),
    )
}

/// Returns geometry-backed clean-family candidates for path-tree evidence.
///
/// These are ordinary unit-cell polygons generated by the existing grid
/// constructors; the production builder, not a synthetic tree fixture, decides
/// whether a candidate is clean and what dual shape it realizes.
#[must_use]
pub fn path_tree_geometry_families(scale: usize) -> Vec<AdversarialInstance> {
    let scale = scale.max(3);
    let mut chain = staircase(scale);
    chain.name = format!("laminar-chain-{scale}");
    "laminar-chain".clone_into(&mut chain.family);
    chain.parameters.insert("scale".to_owned(), scale);

    let mut star = comb(scale, scale.max(3));
    star.name = format!("laminar-star-{scale}");
    "laminar-star".clone_into(&mut star.family);
    star.parameters.insert("scale".to_owned(), scale);

    let mut balanced = comb(scale.saturating_add(1), scale.saturating_add(2));
    balanced.name = format!("balanced-laminar-{scale}");
    "balanced-laminar".clone_into(&mut balanced.family);
    balanced.parameters.insert("scale".to_owned(), scale);

    let mut asymmetric = alternating_notch_corridor(scale);
    asymmetric.name = format!("asymmetric-orientation-{scale}");
    "asymmetric-orientation".clone_into(&mut asymmetric.family);
    asymmetric.parameters.insert("scale".to_owned(), scale);

    vec![chain, star, balanced, asymmetric]
}

fn many_reflex_few_chords(steps: usize) -> AdversarialInstance {
    let width = steps * 2;
    let height = steps + 1;
    let mut cells = vec![false; width * height];
    for step in 0..steps {
        fill_rectangle(&mut cells, width, step * 2, 0, step * 2 + 1, step + 2);
        if step + 1 < steps {
            set_cell(&mut cells, width, step * 2 + 1, step + 1, true);
        }
    }
    instance(
        "many-reflex-few-chords",
        "topology",
        width,
        height,
        cells,
        [("steps".to_owned(), steps)].into_iter().collect(),
    )
}

fn long_collinear_runs(width: usize, height: usize) -> AdversarialInstance {
    let mut cells = vec![true; width * height];
    for y in 1..height - 1 {
        for x in width / 2..width - 1 {
            set_cell(&mut cells, width, x, y, false);
        }
    }
    instance(
        "long-collinear-boundary-runs",
        "topology",
        width,
        height,
        cells,
        [("run_length".to_owned(), width)].into_iter().collect(),
    )
}

fn disconnected_same_color() -> AdversarialInstance {
    let width = 9;
    let height = 5;
    let mut cells = vec![false; width * height];
    fill_rectangle(&mut cells, width, 0, 0, 3, 3);
    fill_rectangle(&mut cells, width, 6, 2, 9, 5);
    instance(
        "disconnected-same-color",
        "connectivity",
        width,
        height,
        cells,
        [("foreground_components".to_owned(), 2)]
            .into_iter()
            .collect(),
    )
}

fn diagonal_touch() -> AdversarialInstance {
    instance(
        "diagonal-touch-only",
        "connectivity",
        2,
        2,
        vec![true, false, false, true],
        [("foreground_components".to_owned(), 2)]
            .into_iter()
            .collect(),
    )
}

fn instance(
    name: &str,
    family: &str,
    width: usize,
    height: usize,
    cells: Vec<bool>,
    parameters: BTreeMap<String, usize>,
) -> AdversarialInstance {
    AdversarialInstance {
        name: name.to_owned(),
        family: family.to_owned(),
        width,
        height,
        cells,
        parameters,
    }
}

fn fill_rectangle(cells: &mut [bool], width: usize, x0: usize, y0: usize, x1: usize, y1: usize) {
    for y in y0..y1 {
        for x in x0..x1 {
            set_cell(cells, width, x, y, true);
        }
    }
}

fn fill_horizontal(cells: &mut [bool], width: usize, x0: usize, x1: usize, y: usize) {
    for x in x0..=x1 {
        set_cell(cells, width, x, y, true);
    }
}

fn fill_vertical(cells: &mut [bool], width: usize, x: usize, y0: usize, y1: usize) {
    for y in y0..=y1 {
        set_cell(cells, width, x, y, true);
    }
}

fn set_cell(cells: &mut [bool], width: usize, x: usize, y: usize, value: bool) {
    cells[y * width + x] = value;
}

#[derive(Debug, Error)]
pub enum AdversarialError {
    #[error("invalid generated grid: {0}")]
    InvalidGrid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use rect_core::closed_chords_intersect;
    use rect_dominance::embedding::{DominanceEmbedding, strict_dominance};
    use rect_oracle_sg::{
        EffectiveChordEnumerator, GridInteriorRunEnumerator, IndexedFrontierCompletion,
        ReferencePairwiseEnumerator, ReferenceRescanCompletion, analyze, complete_with_backend,
    };

    use super::{
        clean_complete_bipartite_grid, contains_complete_bipartite, dense_conflict_grid,
        endpoint_chord_cases, endpoint_contact_instances, external_oracle_adversarial_instances,
        topological_stress_instances,
    };
    use crate::verify_component;
    #[test]
    fn endpoint_cases_preserve_independent_geometry_embedding_equivalence() {
        for case in endpoint_chord_cases() {
            let geometric = closed_chords_intersect(case.horizontal, case.vertical);
            let embedding = DominanceEmbedding::new(&[case.horizontal], &[case.vertical]).unwrap();
            let dominance = strict_dominance(embedding.horizontal[0], embedding.vertical[0]);
            assert_eq!(geometric, case.expected_intersection, "{:?}", case.kind);
            assert_eq!(dominance, geometric, "{:?}", case.kind);
        }
    }

    #[test]
    fn endpoint_component_pairs_are_checked_directly() {
        for instance in endpoint_contact_instances() {
            for component in instance.foreground_components().unwrap() {
                let analysis = rect_oracle_sg::analyze(&component).unwrap();
                let embedding =
                    DominanceEmbedding::new(&analysis.horizontal_chords, &analysis.vertical_chords)
                        .unwrap();
                for (left, &horizontal) in analysis.horizontal_chords.iter().enumerate() {
                    for (right, &vertical) in analysis.vertical_chords.iter().enumerate() {
                        assert_eq!(
                            closed_chords_intersect(horizontal, vertical),
                            strict_dominance(embedding.horizontal[left], embedding.vertical[right]),
                            "{} pair ({left}, {right})",
                            instance.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn completion_backends_match_on_topological_and_dense_adversaries() {
        let instances = endpoint_contact_instances()
            .into_iter()
            .chain(topological_stress_instances())
            .chain(external_oracle_adversarial_instances())
            .chain([dense_conflict_grid(4, 5), dense_conflict_grid(8, 8)]);
        for instance in instances {
            for component in instance.foreground_components().unwrap() {
                let analysis = analyze(&component).unwrap();
                let reference = complete_with_backend(
                    &component,
                    &analysis.horizontal_chords,
                    &analysis.vertical_chords,
                    &analysis.selected_horizontal,
                    &analysis.selected_vertical,
                    &ReferenceRescanCompletion,
                )
                .unwrap();
                let indexed = complete_with_backend(
                    &component,
                    &analysis.horizontal_chords,
                    &analysis.vertical_chords,
                    &analysis.selected_horizontal,
                    &analysis.selected_vertical,
                    &IndexedFrontierCompletion,
                )
                .unwrap();
                assert_eq!(
                    reference.added_horizontal_unit_cuts, indexed.added_horizontal_unit_cuts,
                    "{}",
                    instance.name
                );
                assert_eq!(
                    reference.added_vertical_unit_cuts, indexed.added_vertical_unit_cuts,
                    "{}",
                    instance.name
                );
                assert_eq!(
                    reference.rectangles, indexed.rectangles,
                    "{}",
                    instance.name
                );
                assert_eq!(indexed.metrics.full_grid_vertex_scans, 2);
            }
        }
    }

    #[test]
    fn grid_runs_match_all_adversarial_chord_families() {
        let instances = endpoint_contact_instances()
            .into_iter()
            .chain(topological_stress_instances())
            .chain(external_oracle_adversarial_instances())
            .chain([
                dense_conflict_grid(4, 5),
                dense_conflict_grid(8, 8),
                dense_conflict_grid(32, 32),
            ]);
        for instance in instances {
            for component in instance.foreground_components().unwrap() {
                let boundary = rect_core::Boundary::from_component(&component).unwrap();
                let reference = ReferencePairwiseEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                let grid_runs = GridInteriorRunEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                assert_eq!(
                    reference.horizontal, grid_runs.horizontal,
                    "{}",
                    instance.name
                );
                assert_eq!(reference.vertical, grid_runs.vertical, "{}", instance.name);
            }
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn dense_generator_meets_requested_chord_floor() {
        for (horizontal, vertical) in [(4, 5), (8, 8), (16, 16), (32, 32)] {
            let instance = dense_conflict_grid(horizontal, vertical);
            let component = instance.foreground_components().unwrap().remove(0);
            let analysis = rect_oracle_sg::analyze(&component).unwrap();
            assert!(analysis.horizontal_chords.len() >= horizontal);
            assert!(analysis.vertical_chords.len() >= vertical);
            assert!(contains_complete_bipartite(
                &analysis.conflict_graph,
                horizontal,
                vertical
            ));
            assert!(
                analysis.conflict_graph.edge_count() * 10
                    >= analysis.horizontal_chords.len() * analysis.vertical_chords.len() * 2
            );
            let c0 =
                rect_dominance::solve(&component, rect_dominance::DominanceMode::ExplicitEdges)
                    .unwrap();
            let compact =
                rect_dominance::solve(&component, rect_dominance::DominanceMode::Compact).unwrap();
            let audited_grid_runs =
                rect_dominance::solve_with_verification_mode_and_chord_enumerator(
                    &component,
                    rect_dominance::VerificationMode::FullyAudited,
                    rect_dominance::ChordEnumerator::GridInteriorRuns,
                )
                .unwrap();
            let compact_only = rect_dominance::solve_with_verification_mode(
                &component,
                rect_dominance::VerificationMode::CompactOnly,
            )
            .unwrap();
            assert_eq!(analysis.matching.size, c0.diagnostics.maximum_matching_size);
            assert!(
                c0.diagnostics
                    .execution_trace
                    .pairwise_embedding_audit_called
            );
            assert!(c0.diagnostics.execution_trace.explicit_conflict_graph_built);
            assert!(c0.diagnostics.execution_trace.hopcroft_karp_called);
            assert!(c0.diagnostics.execution_trace.c0_partition_built);
            assert!(
                c0.diagnostics
                    .execution_trace
                    .full_edge_partition_audit_called
            );
            assert_eq!(
                analysis.matching.size,
                compact.diagnostics.maximum_matching_size
            );
            assert_eq!(
                c0.diagnostics.maximum_matching_size,
                compact.diagnostics.maximum_matching_size
            );
            assert_eq!(
                compact.diagnostics.maximum_matching_size,
                compact_only.diagnostics.maximum_matching_size
            );
            assert_eq!(
                compact.optimum_rectangle_count,
                audited_grid_runs.optimum_rectangle_count
            );
            assert_eq!(
                audited_grid_runs
                    .diagnostics
                    .effective_chord_enumerator
                    .as_deref(),
                Some("grid-interior-runs")
            );
            assert!(
                audited_grid_runs
                    .diagnostics
                    .execution_trace
                    .explicit_conflict_graph_built
            );
            assert_eq!(
                compact.optimum_rectangle_count,
                compact_only.optimum_rectangle_count
            );
            assert_eq!(compact_only.diagnostics.explicit_conflict_edge_count, None);
            assert_eq!(
                compact_only.diagnostics.execution_trace,
                rect_core::ExecutionTrace {
                    compact_structure_check_called: true,
                    ..rect_core::ExecutionTrace::default()
                }
            );
            assert_eq!(compact_only.diagnostics.c0_network_vertex_count, 0);
            assert_eq!(compact_only.diagnostics.c0_network_arc_count, 0);
            assert_eq!(
                compact_only.certificate.as_ref().unwrap().payload["verification_mode"],
                "compact-only"
            );
            assert_eq!(
                compact.certificate.as_ref().unwrap().payload["internal_cut_arc_count"],
                0
            );
            assert_eq!(
                compact_only.certificate.as_ref().unwrap().payload["internal_cut_arc_count"],
                0
            );
        }
    }

    #[test]
    fn clean_complete_bipartite_family_is_exact_and_clean() {
        for t in 1..=4 {
            let instance = clean_complete_bipartite_grid(t).unwrap();
            let components = instance.foreground_components().unwrap();
            assert_eq!(components.len(), 1, "t={t}");
            let component = &components[0];
            let geometry =
                rect_oracle_sg::analyze_geometry_with(component, &GridInteriorRunEnumerator)
                    .unwrap();
            let certificate = rect_oracle_sg::classify_clean_hole_free(
                component,
                &geometry.boundary,
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
            );
            assert!(certificate.eligible, "t={t}: {certificate:?}");
            assert_eq!(geometry.horizontal_chords.len(), 2 * t, "t={t}");
            assert_eq!(geometry.vertical_chords.len(), 2 * t, "t={t}");
            let analysis =
                rect_oracle_sg::analyze_with(component, &GridInteriorRunEnumerator).unwrap();
            assert_eq!(analysis.conflict_graph.edge_count(), 4 * t * t, "t={t}");
            assert!(contains_complete_bipartite(
                &analysis.conflict_graph,
                2 * t,
                2 * t
            ));
            let path_tree = rect_dominance::solve_with_representation(
                component,
                rect_dominance::VerificationMode::FullyAudited,
                rect_dominance::ConflictRepresentationBackend::CleanHoleFreePathTree,
                rect_dominance::ChordEnumerator::GridInteriorRuns,
                rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
            );
            assert!(path_tree.is_ok(), "t={t}: {path_tree:?}");
        }
    }

    #[test]
    fn clean_path_tree_compact_execution_remains_edge_free() {
        for t in 1..=3 {
            let instance = clean_complete_bipartite_grid(t).unwrap();
            let component = instance.foreground_components().unwrap().remove(0);
            let result = rect_dominance::solve_with_representation(
                &component,
                rect_dominance::VerificationMode::CompactOnly,
                rect_dominance::ConflictRepresentationBackend::CleanHoleFreePathTree,
                rect_dominance::ChordEnumerator::GridInteriorRuns,
                rect_oracle_sg::CompletionBackendKind::IndexedFrontier,
            )
            .unwrap();
            assert_eq!(result.diagnostics.explicit_conflict_edge_count, None);
            assert_eq!(
                result.diagnostics.execution_trace,
                rect_core::ExecutionTrace {
                    compact_structure_check_called: true,
                    ..rect_core::ExecutionTrace::default()
                }
            );
            assert_eq!(
                result.diagnostics.region_dual_backend.as_deref(),
                Some("boundary-laminar")
            );
            assert_eq!(
                result.diagnostics.explicit_path_records_materialized,
                Some(0)
            );
            assert_eq!(
                result.diagnostics.path_edge_incidence_count,
                Some(4 * t * t)
            );
            assert!(matches!(
                result.diagnostics.path_tree_orientation.as_deref(),
                Some("vertical-tree-horizontal-paths" | "horizontal-tree-vertical-paths")
            ));
        }
    }

    #[test]
    fn topology_suite_is_well_formed_and_deterministic() {
        let first = topological_stress_instances();
        let second = topological_stress_instances();
        assert_eq!(first, second);
        for instance in first {
            assert!(
                !instance.foreground_components().unwrap().is_empty(),
                "{}",
                instance.name
            );
        }
    }

    #[test]
    fn topology_and_dense_suites_agree_across_supported_solvers() {
        let instances = topological_stress_instances()
            .into_iter()
            .chain([dense_conflict_grid(4, 5)]);
        for instance in instances {
            for component in instance.foreground_components().unwrap() {
                let oracle_limit = usize::from(component.cell_count() <= 40) * 40;
                verify_component(&component, oracle_limit)
                    .unwrap_or_else(|error| panic!("{}: {error}", instance.name));
            }
        }
    }
}
