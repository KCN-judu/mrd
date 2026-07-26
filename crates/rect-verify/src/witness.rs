//! Deterministic geometry-backed search for nontrivial path-tree witnesses.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use rect_core::{ColorGrid, Diagnostics, DissectionResult, SvgOverlay, render_dissection_svg};
use rect_dominance::{
    PathTreeOrientation, RegionDualBackend,
    path_tree::build_oriented_path_tree_partition_with_backend_and_options,
};
use rect_oracle_sg::{
    GridInteriorRunEnumerator, analyze_geometry_with, classify_clean_hole_free_with_endpoint_index,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::adversarial::{
    AdversarialInstance, path_tree_geometry_families, topological_stress_instances,
};
use crate::polyomino::enumerate_free_polyominoes;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreeWitness {
    pub name: String,
    pub family: String,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    pub canonical_key: String,
    pub horizontal_chords: usize,
    pub vertical_chords: usize,
    pub dual_max_branching_degree: usize,
    pub path_count: usize,
    pub heavy_chain_interval_count: usize,
    pub paths_using_multiple_heavy_chains: usize,
    pub canonical_segment_node_count: usize,
    pub orientation: String,
    pub boundary: rect_core::Boundary,
    pub dual_tree: rect_dominance::path_tree::RegionDualTree,
    pub compact_paths: Vec<rect_dominance::path_tree::CompactTreePath>,
    pub hld: rect_dominance::path_tree::HeavyLightDecomposition,
    pub biclique_partition: rect_dominance::biclique::BicliquePartition,
    pub diagnostics: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreeWitnessSearchReport {
    pub max_width: usize,
    pub max_height: usize,
    pub seed: u64,
    pub require_clean: bool,
    pub candidates_examined: usize,
    pub witnesses: Vec<PathTreeWitness>,
}

#[derive(Debug, Error)]
pub enum WitnessSearchError {
    #[error("invalid generated witness grid: {0}")]
    Grid(String),
    #[error("witness output failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("witness serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

/// Searches deterministic finite-grid candidates and writes a replayable bundle.
///
/// The predicate is evaluated only through the production boundary-indexed
/// geometry and path-tree builder. No synthetic dual graph is used.
///
/// # Errors
///
/// Returns [`WitnessSearchError`] when a candidate or output bundle cannot be
/// represented or written.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn search_path_tree_witnesses(
    max_width: usize,
    max_height: usize,
    seed: u64,
    require_clean: bool,
    min_horizontal_chords: usize,
    min_vertical_chords: usize,
    min_dual_branching: usize,
    min_path_count: usize,
    min_heavy_chain_intervals: usize,
    min_canonical_nodes: usize,
    output_dir: &Path,
) -> Result<PathTreeWitnessSearchReport, WitnessSearchError> {
    let max_width = max_width.max(1);
    let max_height = max_height.max(1);
    let mut candidates = path_tree_geometry_families(max_width.min(max_height).max(3));
    candidates.extend(topological_stress_instances());
    candidates.extend(structured_notch_families(
        max_width.max(12),
        max_height.max(12),
    ));
    candidates.extend(permuted_notch_grids(max_width.max(24), max_height.max(24)));
    candidates.extend(branch_notch_grids(max_width.max(24), max_height.max(24)));
    candidates.extend(mutated_notch_grids(max_width.max(24), max_height.max(24)));
    let polyomino_limit = max_width.saturating_mul(max_height).min(12);
    for level in enumerate_free_polyominoes(polyomino_limit) {
        for polyomino in level {
            candidates.push(polyomino.to_instance(
                format!("polyomino-{}", polyomino.canonical_key()),
                "free-polyomino",
            ));
        }
    }
    if max_width.saturating_mul(max_height) <= 16 {
        let cells = max_width * max_height;
        for mask in 1_u32..(1_u32 << cells) {
            candidates.push(AdversarialInstance {
                name: format!("exhaustive-{max_width}x{max_height}-{mask:08x}"),
                family: "exhaustive".to_owned(),
                width: max_width,
                height: max_height,
                cells: (0..cells)
                    .map(|index| mask & (1_u32 << index) != 0)
                    .collect(),
                parameters: BTreeMap::new(),
            });
        }
    }
    let mut random = SplitMix64::new(seed);
    let random_cases = 512;
    for case in 0..random_cases {
        candidates.push(random_connected_instance(
            max_width,
            max_height,
            &mut random,
            case,
        ));
        candidates.push(random_boundary_notch_instance(
            max_width.max(8),
            max_height.max(8),
            &mut random,
            case,
        ));
    }

    let mut examined = 0;
    let mut witnesses = Vec::new();
    let mut seen_keys = BTreeSet::<String>::new();
    for candidate in candidates {
        let grid = ColorGrid::new(candidate.width, candidate.height, candidate.cells.clone())
            .map_err(|error| WitnessSearchError::Grid(error.to_string()))?;
        for component in grid
            .four_connected_components()
            .into_iter()
            .filter(|c| c.color)
        {
            examined += 1;
            let Ok(geometry) = analyze_geometry_with(&component, &GridInteriorRunEnumerator) else {
                continue;
            };
            if geometry.horizontal_chords.len() < min_horizontal_chords
                || geometry.vertical_chords.len() < min_vertical_chords
            {
                continue;
            }
            let certificate = classify_clean_hole_free_with_endpoint_index(
                &component,
                &geometry.boundary,
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
                &geometry.endpoint_index,
            );
            if require_clean && !certificate.eligible {
                continue;
            }
            if !certificate.eligible {
                continue;
            }
            let Some((_orientation, partition)) = [
                PathTreeOrientation::VerticalTreeHorizontalPaths,
                PathTreeOrientation::HorizontalTreeVerticalPaths,
            ]
            .into_iter()
            .find_map(|orientation| {
                build_oriented_path_tree_partition_with_backend_and_options(
                    &geometry.prepared,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                    certificate.clone(),
                    orientation,
                    false,
                    RegionDualBackend::BoundaryLaminar,
                    Some(&geometry.endpoint_index),
                    rect_dominance::BoundaryGapLabelBackend::EventSweep,
                )
                .ok()
                .and_then(|partition| {
                    let branching = partition
                        .path_tree
                        .tree
                        .adjacency
                        .iter()
                        .map(Vec::len)
                        .max()
                        .unwrap_or(0);
                    let mut intervals = 0;
                    let mut multi_chain = 0;
                    for path in &partition.path_tree.compact_paths {
                        let count = partition
                            .path_tree
                            .hld
                            .decompose_path_endpoints(path.start_region, path.end_region)
                            .map(|items| items.len())
                            .unwrap_or(0);
                        intervals += count;
                        multi_chain += usize::from(count >= 2);
                    }
                    (branching >= min_dual_branching
                        && partition.path_count >= min_path_count
                        && intervals >= min_heavy_chain_intervals
                        && multi_chain > 0
                        && partition.canonical_segment_node_count >= min_canonical_nodes)
                        .then_some((orientation, partition))
                })
            }) else {
                continue;
            };
            let max_branching = partition
                .path_tree
                .tree
                .adjacency
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(0);
            let mut heavy_intervals = 0;
            let mut multi_chain_paths = 0;
            for path in &partition.path_tree.compact_paths {
                let count = partition
                    .path_tree
                    .hld
                    .decompose_path_endpoints(path.start_region, path.end_region)
                    .map(|intervals| intervals.len())
                    .unwrap_or(0);
                heavy_intervals += count;
                multi_chain_paths += usize::from(count >= 2);
            }
            if max_branching < min_dual_branching
                || partition.path_count < min_path_count
                || heavy_intervals < min_heavy_chain_intervals
                || multi_chain_paths == 0
                || partition.canonical_segment_node_count < min_canonical_nodes
            {
                continue;
            }
            let (canonical_key, canonical_cells, canonical_width, canonical_height) =
                canonical_cells(&component);
            if !seen_keys.insert(canonical_key.clone()) {
                continue;
            }
            // Canonicalization changes coordinates. Rebuild all geometry and
            // certificates on the canonical cells so the persisted witness
            // is replayable rather than mixing transformed cells with the
            // source component's boundary metadata.
            let canonical_grid =
                ColorGrid::new(canonical_width, canonical_height, canonical_cells.clone())
                    .map_err(|error| WitnessSearchError::Grid(error.to_string()))?;
            let Some(canonical_component) = canonical_grid
                .four_connected_components()
                .into_iter()
                .find(|candidate| candidate.color)
            else {
                continue;
            };
            let Ok(canonical_geometry) =
                analyze_geometry_with(&canonical_component, &GridInteriorRunEnumerator)
            else {
                continue;
            };
            let canonical_certificate = classify_clean_hole_free_with_endpoint_index(
                &canonical_component,
                &canonical_geometry.boundary,
                &canonical_geometry.horizontal_chords,
                &canonical_geometry.vertical_chords,
                &canonical_geometry.endpoint_index,
            );
            if !canonical_certificate.eligible
                || canonical_geometry.horizontal_chords.len() < min_horizontal_chords
                || canonical_geometry.vertical_chords.len() < min_vertical_chords
            {
                continue;
            }
            let Some((orientation, partition)) = [
                PathTreeOrientation::VerticalTreeHorizontalPaths,
                PathTreeOrientation::HorizontalTreeVerticalPaths,
            ]
            .into_iter()
            .find_map(|orientation| {
                build_oriented_path_tree_partition_with_backend_and_options(
                    &canonical_geometry.prepared,
                    &canonical_geometry.boundary,
                    &canonical_geometry.horizontal_chords,
                    &canonical_geometry.vertical_chords,
                    canonical_certificate.clone(),
                    orientation,
                    false,
                    RegionDualBackend::BoundaryLaminar,
                    Some(&canonical_geometry.endpoint_index),
                    rect_dominance::BoundaryGapLabelBackend::EventSweep,
                )
                .ok()
                .and_then(|partition| {
                    let branching = partition
                        .path_tree
                        .tree
                        .adjacency
                        .iter()
                        .map(Vec::len)
                        .max()
                        .unwrap_or(0);
                    let mut intervals = 0;
                    let mut multi_chain = 0;
                    for path in &partition.path_tree.compact_paths {
                        let count = partition
                            .path_tree
                            .hld
                            .decompose_path_endpoints(path.start_region, path.end_region)
                            .map(|items| items.len())
                            .unwrap_or(0);
                        intervals += count;
                        multi_chain += usize::from(count >= 2);
                    }
                    (branching >= min_dual_branching
                        && partition.path_count >= min_path_count
                        && intervals >= min_heavy_chain_intervals
                        && multi_chain > 0
                        && partition.canonical_segment_node_count >= min_canonical_nodes)
                        .then_some((orientation, partition))
                })
            }) else {
                continue;
            };
            let canonical_max_branching = partition
                .path_tree
                .tree
                .adjacency
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(0);
            let mut canonical_heavy_intervals = 0;
            let mut canonical_multi_chain_paths = 0;
            for path in &partition.path_tree.compact_paths {
                let count = partition
                    .path_tree
                    .hld
                    .decompose_path_endpoints(path.start_region, path.end_region)
                    .map(|intervals| intervals.len())
                    .unwrap_or(0);
                canonical_heavy_intervals += count;
                canonical_multi_chain_paths += usize::from(count >= 2);
            }
            if canonical_max_branching < min_dual_branching
                || partition.path_count < min_path_count
                || canonical_heavy_intervals < min_heavy_chain_intervals
                || canonical_multi_chain_paths == 0
                || partition.canonical_segment_node_count < min_canonical_nodes
            {
                continue;
            }
            let mut diagnostics = BTreeMap::new();
            diagnostics.insert(
                "boundary_complexity".to_owned(),
                canonical_geometry.boundary.boundary_complexity(),
            );
            diagnostics.insert(
                "reflex_vertices".to_owned(),
                canonical_geometry.boundary.reflex_vertices.len(),
            );
            diagnostics.insert("dual_regions".to_owned(), partition.dual_region_count);
            diagnostics.insert(
                "boundary_gap_event_push_count".to_owned(),
                partition.path_tree.tree.boundary_gap_event_push_count,
            );
            diagnostics.insert(
                "boundary_gap_event_pop_count".to_owned(),
                partition.path_tree.tree.boundary_gap_event_pop_count,
            );
            let witness = PathTreeWitness {
                name: candidate.name.clone(),
                family: candidate.family.clone(),
                width: canonical_width,
                height: canonical_height,
                cells: canonical_cells,
                canonical_key,
                horizontal_chords: canonical_geometry.horizontal_chords.len(),
                vertical_chords: canonical_geometry.vertical_chords.len(),
                dual_max_branching_degree: canonical_max_branching,
                path_count: partition.path_count,
                heavy_chain_interval_count: canonical_heavy_intervals,
                paths_using_multiple_heavy_chains: canonical_multi_chain_paths,
                canonical_segment_node_count: partition.canonical_segment_node_count,
                orientation: orientation.name().to_owned(),
                boundary: canonical_geometry.boundary,
                dual_tree: partition.path_tree.tree.clone(),
                compact_paths: partition.path_tree.compact_paths.clone(),
                hld: partition.path_tree.hld.clone(),
                biclique_partition: partition.biclique_partition,
                diagnostics,
            };
            witnesses.push(witness);
            if witnesses.len() >= 16 {
                break;
            }
        }
        if witnesses.len() >= 16 {
            break;
        }
    }
    witnesses.sort_by_key(|w| {
        (
            w.cells.iter().filter(|&&cell| cell).count(),
            w.width,
            w.height,
            w.canonical_key.clone(),
        )
    });
    fs::create_dir_all(output_dir)?;
    for (index, witness) in witnesses.iter().enumerate() {
        let stem = format!("witness-{index:03}-{}", short_hash(&witness.canonical_key));
        fs::write(
            output_dir.join(format!("{stem}.json")),
            serde_json::to_vec_pretty(witness)?,
        )?;
        let grid = ColorGrid::new(witness.width, witness.height, witness.cells.clone())
            .map_err(|error| WitnessSearchError::Grid(error.to_string()))?;
        if let Some(component) = grid
            .four_connected_components()
            .into_iter()
            .find(|c| c.color)
        {
            let geometry = analyze_geometry_with(&component, &GridInteriorRunEnumerator)
                .map_err(|error| WitnessSearchError::Grid(error.to_string()))?;
            let result = DissectionResult {
                optimum_rectangle_count: 0,
                rectangles: Vec::new(),
                diagnostics: Diagnostics::default(),
                certificate: None,
            };
            let svg = render_dissection_svg(
                &component,
                &result,
                &SvgOverlay {
                    horizontal_chords: &geometry.horizontal_chords,
                    vertical_chords: &geometry.vertical_chords,
                    selected_horizontal: &vec![true; geometry.horizontal_chords.len()],
                    selected_vertical: &vec![true; geometry.vertical_chords.len()],
                },
            )
            .map_err(|error| WitnessSearchError::Grid(error.to_string()))?;
            fs::write(output_dir.join(format!("{stem}.svg")), svg)?;
        }
    }
    let report = PathTreeWitnessSearchReport {
        max_width,
        max_height,
        seed,
        require_clean,
        candidates_examined: examined,
        witnesses,
    };
    fs::write(
        output_dir.join("index.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}

fn random_connected_instance(
    width: usize,
    height: usize,
    random: &mut SplitMix64,
    case: usize,
) -> AdversarialInstance {
    let mut cells = vec![false; width * height];
    let mut x = random.index(width);
    let mut y = random.index(height);
    cells[y * width + x] = true;
    let steps = (width * height).saturating_mul(2).max(1);
    for _ in 0..steps {
        match random.index(4) {
            0 if x > 0 => x -= 1,
            1 if x + 1 < width => x += 1,
            2 if y > 0 => y -= 1,
            3 if y + 1 < height => y += 1,
            _ => {}
        }
        cells[y * width + x] = true;
    }
    AdversarialInstance {
        name: format!("random-connected-{case:04}"),
        family: "random-connected".to_owned(),
        width,
        height,
        cells,
        parameters: [("case".to_owned(), case)].into_iter().collect(),
    }
}

fn random_boundary_notch_instance(
    width: usize,
    height: usize,
    random: &mut SplitMix64,
    case: usize,
) -> AdversarialInstance {
    let mut cells = vec![true; width * height];
    let notch_count = 2 + random.index(8);
    for _ in 0..notch_count {
        let x = 1 + random.index(width.saturating_sub(2).max(1));
        let depth_bound = height.saturating_sub(3).clamp(1, 5);
        let depth = 1 + random.index(depth_bound);
        if random.index(2) == 0 {
            for y in height.saturating_sub(depth)..height.saturating_sub(1) {
                cells[y * width + x] = false;
            }
        } else {
            for y in 1..=depth.min(height.saturating_sub(2)) {
                cells[y * width + x] = false;
            }
        }
    }
    for _ in 0..notch_count {
        let y = 1 + random.index(height.saturating_sub(2).max(1));
        let depth_bound = width.saturating_sub(3).clamp(1, 5);
        let depth = 1 + random.index(depth_bound);
        if random.index(2) == 0 {
            for x in width.saturating_sub(depth)..width.saturating_sub(1) {
                cells[y * width + x] = false;
            }
        } else {
            for x in 1..=depth.min(width.saturating_sub(2)) {
                cells[y * width + x] = false;
            }
        }
    }
    AdversarialInstance {
        name: format!("random-boundary-notches-{case:04}"),
        family: "random-boundary-notches".to_owned(),
        width,
        height,
        cells,
        parameters: [("notches".to_owned(), notch_count)].into_iter().collect(),
    }
}

fn structured_notch_families(width: usize, height: usize) -> Vec<AdversarialInstance> {
    let xs = [2, width / 3, width / 2, width.saturating_sub(3)];
    let ys = [2, height / 3, height / 2, height.saturating_sub(3)];
    let mut families = Vec::new();
    for variant in 0..16 {
        let mut cells = vec![true; width * height];
        for (index, &x) in xs.iter().enumerate() {
            let depth = 2 + (variant + index) % 4;
            if x < width {
                for y in height.saturating_sub(depth)..height {
                    cells[y * width + x] = false;
                }
            }
            if variant & (1 << index) != 0 && x + 1 < width {
                for y in 0..depth {
                    cells[y * width + x + 1] = false;
                }
            }
        }
        for (index, &y) in ys.iter().enumerate() {
            let depth = 2 + (variant + 2 * index) % 4;
            if y < height {
                for x in width.saturating_sub(depth)..width {
                    cells[y * width + x] = false;
                }
            }
            if variant & (1 << ((index + 4) % 8)) != 0 && y + 1 < height {
                for x in 0..depth {
                    cells[(y + 1) * width + x] = false;
                }
            }
        }
        families.push(AdversarialInstance {
            name: format!("structured-notch-{variant:02}"),
            family: "structured-boundary-notches".to_owned(),
            width,
            height,
            cells,
            parameters: [("variant".to_owned(), variant)].into_iter().collect(),
        });
    }
    families
}

fn permuted_notch_grids(width: usize, height: usize) -> Vec<AdversarialInstance> {
    let margin = 4;
    let positions = [margin, margin + 4, margin + 8, margin + 12, margin + 16];
    let depths = [
        [2, 4, 6, 8, 3],
        [8, 6, 4, 2, 7],
        [2, 6, 3, 7, 4],
        [7, 3, 6, 2, 8],
        [4, 8, 2, 6, 3],
        [6, 2, 8, 4, 7],
    ];
    let mut result = Vec::new();
    for variant in 0..depths.len() {
        let mut cells = vec![true; width * height];
        let d = depths[variant];
        for (index, &x) in positions.iter().enumerate() {
            for y in height.saturating_sub(d[index])..height {
                if x < width {
                    cells[y * width + x] = false;
                }
            }
            for y in 0..d[(index + variant) % d.len()] {
                if x + 1 < width {
                    cells[y * width + x + 1] = false;
                }
            }
        }
        for (index, &y) in positions.iter().enumerate() {
            for x in width.saturating_sub(d[index])..width {
                if y < height {
                    cells[y * width + x] = false;
                }
            }
            for x in 0..d[(index + 1 + variant) % d.len()] {
                if y + 1 < height {
                    cells[(y + 1) * width + x] = false;
                }
            }
        }
        result.push(AdversarialInstance {
            name: format!("permuted-notch-grid-{variant}"),
            family: "permuted-notch-grid".to_owned(),
            width,
            height,
            cells,
            parameters: [("variant".to_owned(), variant)].into_iter().collect(),
        });
    }
    let mut random = SplitMix64::new(0x7061_7468_2d76_3038);
    for variant in depths.len()..1024 {
        let generated = [
            2 + random.index(9),
            2 + random.index(9),
            2 + random.index(9),
            2 + random.index(9),
            2 + random.index(9),
        ];
        let mut cells = vec![true; width * height];
        for (index, &x) in positions.iter().enumerate() {
            let depth = generated[index];
            for y in height.saturating_sub(depth)..height {
                if x < width {
                    cells[y * width + x] = false;
                }
            }
            for y in 0..generated[(index + 1) % generated.len()] {
                if x + 1 < width {
                    cells[y * width + x + 1] = false;
                }
            }
        }
        for (index, &y) in positions.iter().enumerate() {
            let depth = generated[(index + 2) % generated.len()];
            for x in width.saturating_sub(depth)..width {
                if y < height {
                    cells[y * width + x] = false;
                }
            }
            for x in 0..generated[(index + 3) % generated.len()] {
                if y + 1 < height {
                    cells[(y + 1) * width + x] = false;
                }
            }
        }
        result.push(AdversarialInstance {
            name: format!("permuted-notch-grid-{variant}"),
            family: "permuted-notch-grid".to_owned(),
            width,
            height,
            cells,
            parameters: [("variant".to_owned(), variant)].into_iter().collect(),
        });
    }
    result
}

fn branch_notch_grids(width: usize, height: usize) -> Vec<AdversarialInstance> {
    let positions = [4, 8, 12, 16];
    let depth_sets = [[2, 4, 6, 8], [8, 6, 4, 2], [3, 7, 4, 6], [6, 3, 7, 4]];
    let mut result = Vec::new();
    for (variant, depths) in depth_sets.into_iter().enumerate() {
        let mut cells = vec![true; width * height];
        for (index, &x) in positions.iter().enumerate() {
            let depth = depths[index];
            for y in height.saturating_sub(depth)..height {
                if x < width {
                    cells[y * width + x] = false;
                }
            }
        }
        for (index, &y) in positions.iter().enumerate() {
            let depth = depths[(index + variant) % depths.len()];
            for x in 0..depth {
                if y < height {
                    cells[y * width + x] = false;
                }
            }
        }
        result.push(AdversarialInstance {
            name: format!("branch-notch-grid-{variant}"),
            family: "branch-notch-grid".to_owned(),
            width,
            height,
            cells,
            parameters: [("variant".to_owned(), variant)].into_iter().collect(),
        });
    }
    result
}

fn mutated_notch_grids(width: usize, height: usize) -> Vec<AdversarialInstance> {
    let bases = permuted_notch_grids(width, height);
    let mut random = SplitMix64::new(0x6d75_7461_7465_647f);
    let mut result = Vec::new();
    for (base_index, base) in bases.into_iter().take(128).enumerate() {
        for mutation in 0..128 {
            let mut cells = base.cells.clone();
            let index = random.index(cells.len());
            cells[index] = !cells[index];
            result.push(AdversarialInstance {
                name: format!("mutated-notch-{base_index:03}-{mutation:03}"),
                family: "mutated-notch-grid".to_owned(),
                width,
                height,
                cells,
                parameters: [
                    ("base".to_owned(), base_index),
                    ("mutation".to_owned(), mutation),
                ]
                .into_iter()
                .collect(),
            });
        }
    }
    result
}

fn canonical_cells(
    component: &rect_core::GridComponent<bool>,
) -> (String, Vec<bool>, usize, usize) {
    let variants = (0..8).map(|symmetry| {
        let swap = symmetry >= 4;
        let source_width = component.grid_width;
        let source_height = component.grid_height;
        let width = if swap { source_height } else { source_width };
        let height = if swap { source_width } else { source_height };
        let mut cells = vec![false; width * height];
        for cell in &component.cells {
            let (mut x, mut y) = (cell.x, cell.y);
            if symmetry % 4 == 1 {
                (x, y) = (source_height - 1 - y, x);
            } else if symmetry % 4 == 2 {
                (x, y) = (source_width - 1 - x, source_height - 1 - y);
            } else if symmetry % 4 == 3 {
                (x, y) = (y, source_width - 1 - x);
            }
            if symmetry >= 4 {
                x = width - 1 - x;
            }
            if x < width && y < height {
                cells[y * width + x] = true;
            }
        }
        let key = cells
            .iter()
            .map(|cell| if *cell { '1' } else { '0' })
            .collect::<String>();
        (key, cells, width, height)
    });
    variants
        .min_by_key(|variant| (variant.0.clone(), variant.2, variant.3))
        .unwrap()
}

fn short_hash(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, upper: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(upper.max(1)).unwrap()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::search_path_tree_witnesses;

    #[test]
    fn deterministic_search_finds_geometry_backed_mixed_branching_witness() {
        let output =
            std::env::temp_dir().join(format!("mrd-path-tree-witness-{}", std::process::id()));
        let report =
            search_path_tree_witnesses(12, 12, 42, true, 2, 2, 3, 3, 4, 2, &output).unwrap();
        assert!(report.witnesses.iter().any(|witness| {
            witness.dual_max_branching_degree >= 3
                && witness.path_count >= 3
                && witness.paths_using_multiple_heavy_chains > 0
                && witness.canonical_segment_node_count >= 2
        }));
        fs::remove_dir_all(output).unwrap();
    }
}
