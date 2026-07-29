//! Differential verification for boundary-gap labeling backends.

use std::collections::BTreeMap;
use std::fmt::Write;

use dominance::experiment::{
    ChordEnumerator, GapBackend, PathTreeOrientation, PathTreeOrientationPolicy, RegionBackend,
    Representation, Verification, solve_with_representation_and_path_tree_options,
};
use dominance::path_tree::build_oriented_path_tree_partition_with_backend_and_options;
use mrd_domain::{ColorGrid, GridComponent, validate_dissection};
use serde::{Deserialize, Serialize};
use sg_oracle::grid::experiment::InteriorRuns;
use sg_oracle::grid::{
    CompletionBackendKind, analyze_geometry_with, classify_clean_hole_free_with_endpoint_index,
};

use crate::adversarial::{
    AdversarialInstance, clean_complete_bipartite_grid, path_tree_geometry_families,
    topological_stress_instances,
};
use crate::benchmark::{BenchmarkContext, BenchmarkMetadata};
use crate::polyomino::enumerate_free_polyominoes;
use crate::transforms::{GridTransform, TransformedComponent};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GapDifferentialConfig {
    pub exhaustive_sides: Vec<usize>,
    pub polyomino_max_cells: usize,
    pub random_width: usize,
    pub random_height: usize,
    pub random_cases: usize,
    pub random_seed: u64,
    pub complete_bipartite_max_t: usize,
    pub family_scales: Vec<usize>,
    pub include_dihedral_transforms: bool,
}

impl Default for GapDifferentialConfig {
    fn default() -> Self {
        Self {
            exhaustive_sides: vec![3, 4],
            polyomino_max_cells: 12,
            random_width: 12,
            random_height: 12,
            random_cases: 100_000,
            random_seed: 42,
            complete_bipartite_max_t: 128,
            family_scales: vec![3, 8, 16, 32, 64, 128],
            include_dihedral_transforms: true,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GapDifferentialPopulation {
    pub population: String,
    pub input_count: usize,
    pub component_count: usize,
    pub clean_component_count: usize,
    pub ineligible_component_count: usize,
    pub boundary_index_comparison_count: usize,
    pub endpoint_metadata_comparison_count: usize,
    pub clean_classifier_comparison_count: usize,
    pub orientation_comparison_count: usize,
    pub verified_component_count: usize,
    pub mismatch_count: usize,
    pub solver_error_count: usize,
    pub nested_membership_tests: usize,
    pub event_push_count: usize,
    pub event_pop_count: usize,
    pub maximum_q: usize,
    pub maximum_boundary_complexity: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GapDifferentialFailure {
    pub population: String,
    pub instance_name: String,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    pub minimized_width: usize,
    pub minimized_height: usize,
    pub minimized_cells: Vec<bool>,
    pub differences: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GapDifferentialReport {
    pub metadata: BenchmarkMetadata,
    pub config: GapDifferentialConfig,
    pub populations: Vec<GapDifferentialPopulation>,
    pub total_input_count: usize,
    pub total_component_count: usize,
    pub total_clean_component_count: usize,
    pub total_boundary_index_comparison_count: usize,
    pub total_endpoint_metadata_comparison_count: usize,
    pub total_clean_classifier_comparison_count: usize,
    pub total_orientation_comparison_count: usize,
    pub total_verified_component_count: usize,
    pub total_mismatch_count: usize,
    pub total_solver_error_count: usize,
    pub total_nested_membership_tests: usize,
    pub total_event_push_count: usize,
    pub total_event_pop_count: usize,
    pub failures: Vec<GapDifferentialFailure>,
}

impl GapDifferentialReport {
    #[must_use]
    pub fn verified(&self) -> bool {
        self.total_mismatch_count == 0 && self.total_solver_error_count == 0
    }

    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "population,input_count,component_count,clean_component_count,ineligible_component_count,boundary_index_comparison_count,endpoint_metadata_comparison_count,clean_classifier_comparison_count,orientation_comparison_count,verified_component_count,mismatch_count,solver_error_count,nested_membership_tests,event_push_count,event_pop_count,maximum_q,maximum_boundary_complexity\n",
        );
        for population in &self.populations {
            let _ = writeln!(
                csv,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                population.population,
                population.input_count,
                population.component_count,
                population.clean_component_count,
                population.ineligible_component_count,
                population.boundary_index_comparison_count,
                population.endpoint_metadata_comparison_count,
                population.clean_classifier_comparison_count,
                population.orientation_comparison_count,
                population.verified_component_count,
                population.mismatch_count,
                population.solver_error_count,
                population.nested_membership_tests,
                population.event_push_count,
                population.event_pop_count,
                population.maximum_q,
                population.maximum_boundary_complexity,
            );
        }
        csv
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!(
            "# v0.8 indexed frontend and boundary-gap differential\n\n- Inputs: {}\n- Components: {}\n- Clean components: {}\n- Boundary-index comparisons: {}\n- Endpoint-metadata comparisons: {}\n- Clean-classifier comparisons: {}\n- Orientation comparisons: {}\n- Verified clean components: {}\n- Mismatches: {}\n- Solver errors: {}\n- Nested membership tests: {}\n- Event pushes/pops: {}/{}\n",
            self.total_input_count,
            self.total_component_count,
            self.total_clean_component_count,
            self.total_boundary_index_comparison_count,
            self.total_endpoint_metadata_comparison_count,
            self.total_clean_classifier_comparison_count,
            self.total_orientation_comparison_count,
            self.total_verified_component_count,
            self.total_mismatch_count,
            self.total_solver_error_count,
            self.total_nested_membership_tests,
            self.total_event_push_count,
            self.total_event_pop_count,
        )
    }
}

#[derive(Clone, Debug)]
struct ComponentEvidence {
    clean: bool,
    boundary_index_comparisons: usize,
    endpoint_metadata_comparisons: usize,
    clean_classifier_comparisons: usize,
    orientation_comparisons: usize,
    nested_membership_tests: usize,
    event_push_count: usize,
    event_pop_count: usize,
    q: usize,
    boundary_complexity: usize,
}

#[allow(clippy::too_many_lines)]
fn compare_clean_component(
    component: &GridComponent<bool>,
) -> Result<ComponentEvidence, Vec<String>> {
    let geometry = analyze_geometry_with(component, &InteriorRuns)
        .map_err(|error| vec![format!("geometry: {error}")])?;
    let mut differences = Vec::new();
    let mut boundary_index_comparisons = 0usize;
    for boundary_loop in &geometry.boundary.loops {
        for &point in &boundary_loop.vertices {
            boundary_index_comparisons += 1;
            if geometry.boundary_index.vertex_id(point) != geometry.boundary.vertex_id(point) {
                differences.push("boundary index versus linear lookup".to_owned());
                break;
            }
        }
    }
    let mut endpoint_metadata_comparisons = 0usize;
    for (index, &chord) in geometry.horizontal_chords.iter().enumerate() {
        endpoint_metadata_comparisons += 1;
        if sg_oracle::grid::horizontal_chord_endpoints(&geometry.boundary, chord).ok()
            != geometry.endpoint_index.horizontal.get(index).copied()
        {
            differences.push(format!("horizontal endpoint metadata {index}"));
        }
    }
    for (index, &chord) in geometry.vertical_chords.iter().enumerate() {
        endpoint_metadata_comparisons += 1;
        if sg_oracle::grid::vertical_chord_endpoints(&geometry.boundary, chord).ok()
            != geometry.endpoint_index.vertical.get(index).copied()
        {
            differences.push(format!("vertical endpoint metadata {index}"));
        }
    }
    let certificate = classify_clean_hole_free_with_endpoint_index(
        component,
        &geometry.boundary,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        &geometry.endpoint_index,
    );
    let reference_certificate = sg_oracle::grid::classify_clean_hole_free_reference(
        component,
        &geometry.boundary,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
    );
    let canonical_certificate = |mut value: sg_oracle::grid::CleanHoleFreeCertificate| {
        value
            .rejection_reasons
            .sort_by_key(|reason| format!("{reason:?}"));
        value
    };
    if canonical_certificate(certificate.clone()) != canonical_certificate(reference_certificate) {
        differences.push("indexed versus pairwise clean classifier".to_owned());
    }
    if !differences.is_empty() {
        return Err(differences);
    }
    if !certificate.eligible {
        return Ok(ComponentEvidence {
            clean: false,
            boundary_index_comparisons,
            endpoint_metadata_comparisons,
            clean_classifier_comparisons: 1,
            orientation_comparisons: 0,
            nested_membership_tests: 0,
            event_push_count: 0,
            event_pop_count: 0,
            q: geometry.horizontal_chords.len() + geometry.vertical_chords.len(),
            boundary_complexity: geometry.boundary.boundary_complexity(),
        });
    }

    let mut nested_membership_tests = 0usize;
    let mut event_push_count = 0usize;
    let mut event_pop_count = 0usize;
    for orientation in [
        PathTreeOrientation::VerticalTreeHorizontalPaths,
        PathTreeOrientation::HorizontalTreeVerticalPaths,
    ] {
        let reference = build_oriented_path_tree_partition_with_backend_and_options(
            &geometry.prepared,
            &geometry.boundary,
            &geometry.horizontal_chords,
            &geometry.vertical_chords,
            certificate.clone(),
            orientation,
            false,
            RegionBackend::Experiment,
            Some(&geometry.endpoint_index),
            GapBackend::Oracle,
        );
        let event = build_oriented_path_tree_partition_with_backend_and_options(
            &geometry.prepared,
            &geometry.boundary,
            &geometry.horizontal_chords,
            &geometry.vertical_chords,
            certificate.clone(),
            orientation,
            false,
            RegionBackend::Experiment,
            Some(&geometry.endpoint_index),
            GapBackend::Experiment,
        );
        let (reference, event) = match (reference, event) {
            (Ok(reference), Ok(event)) => (reference, event),
            (Err(error), _) => {
                differences.push(format!(
                    "{} reference builder error: {error}",
                    orientation.name()
                ));
                continue;
            }
            (_, Err(error)) => {
                differences.push(format!(
                    "{} event builder error: {error}",
                    orientation.name()
                ));
                continue;
            }
        };
        nested_membership_tests = nested_membership_tests
            .saturating_add(reference.path_tree.tree.boundary_gap_membership_tests);
        event_push_count =
            event_push_count.saturating_add(event.path_tree.tree.boundary_gap_event_push_count);
        event_pop_count =
            event_pop_count.saturating_add(event.path_tree.tree.boundary_gap_event_pop_count);
        let prefix = orientation.name();
        if reference.path_tree.tree.edges != event.path_tree.tree.edges {
            differences.push(format!("{prefix}: dual chord-labeled edges"));
        }
        if reference.path_tree.tree.boundary_gap_regions
            != event.path_tree.tree.boundary_gap_regions
        {
            differences.push(format!("{prefix}: boundary gap regions"));
        }
        if reference.path_tree.compact_paths != event.path_tree.compact_paths {
            differences.push(format!("{prefix}: endpoint regions/compact paths"));
        }
        if reference.path_tree.hld != event.path_tree.hld {
            differences.push(format!("{prefix}: HLD arrays and chain intervals"));
        }
        if reference.biclique_partition != event.biclique_partition {
            differences.push(format!("{prefix}: biclique partition"));
        }
        if event.path_tree.tree.boundary_gap_membership_tests != 0 {
            differences.push(format!("{prefix}: event membership tests are nonzero"));
        }
        if event.path_tree.tree.boundary_gap_event_push_count != event.path_tree.tree.edges.len()
            || event.path_tree.tree.boundary_gap_event_pop_count != event.path_tree.tree.edges.len()
        {
            differences.push(format!(
                "{prefix}: event counts are not one push/pop per interval"
            ));
        }
    }

    let reference_result = solve_with_representation_and_path_tree_options(
        component,
        Verification::CompactOnly,
        Representation::CleanHoleFreePathTree,
        ChordEnumerator::GridInteriorRuns,
        CompletionBackendKind::IndexedFrontier,
        RegionBackend::Experiment,
        PathTreeOrientationPolicy::BuildBothExact,
        GapBackend::Oracle,
    );
    let event_result = solve_with_representation_and_path_tree_options(
        component,
        Verification::CompactOnly,
        Representation::CleanHoleFreePathTree,
        ChordEnumerator::GridInteriorRuns,
        CompletionBackendKind::IndexedFrontier,
        RegionBackend::Experiment,
        PathTreeOrientationPolicy::BuildBothExact,
        GapBackend::Experiment,
    );
    match (reference_result, event_result) {
        (Ok(reference), Ok(event)) => {
            if validate_dissection(component, &reference).is_err()
                || validate_dissection(component, &event).is_err()
            {
                differences.push("cell-exact output validation".to_owned());
            }
            if reference.optimum_rectangle_count != event.optimum_rectangle_count {
                differences.push("final optimum".to_owned());
            }
            if reference.rectangles != event.rectangles {
                differences.push("final rectangles".to_owned());
            }
            if reference.diagnostics.boundary_gap_label_backend.as_deref()
                != Some(GapBackend::Oracle.name())
                || event.diagnostics.boundary_gap_label_backend.as_deref()
                    != Some(GapBackend::Experiment.name())
            {
                differences.push("solver backend diagnostic".to_owned());
            }
        }
        (Err(error), _) => differences.push(format!("reference solver error: {error}")),
        (_, Err(error)) => differences.push(format!("event solver error: {error}")),
    }

    if differences.is_empty() {
        Ok(ComponentEvidence {
            clean: true,
            boundary_index_comparisons,
            endpoint_metadata_comparisons,
            clean_classifier_comparisons: 1,
            orientation_comparisons: 2,
            nested_membership_tests,
            event_push_count,
            event_pop_count,
            q: geometry.horizontal_chords.len() + geometry.vertical_chords.len(),
            boundary_complexity: geometry.boundary.boundary_complexity(),
        })
    } else {
        Err(differences)
    }
}

fn evaluate_instance(
    population: &mut GapDifferentialPopulation,
    failures: &mut Vec<GapDifferentialFailure>,
    instance: &AdversarialInstance,
) {
    population.input_count += 1;
    let Ok(grid) = ColorGrid::new(instance.width, instance.height, instance.cells.clone()) else {
        population.solver_error_count += 1;
        return;
    };
    for component in grid
        .four_connected_components()
        .into_iter()
        .filter(|component| component.color)
    {
        population.component_count += 1;
        match compare_clean_component(&component) {
            Ok(evidence) => {
                population.boundary_index_comparison_count += evidence.boundary_index_comparisons;
                population.endpoint_metadata_comparison_count +=
                    evidence.endpoint_metadata_comparisons;
                population.clean_classifier_comparison_count +=
                    evidence.clean_classifier_comparisons;
                population.maximum_q = population.maximum_q.max(evidence.q);
                population.maximum_boundary_complexity = population
                    .maximum_boundary_complexity
                    .max(evidence.boundary_complexity);
                if !evidence.clean {
                    population.ineligible_component_count += 1;
                    continue;
                }
                population.clean_component_count += 1;
                population.orientation_comparison_count += evidence.orientation_comparisons;
                population.verified_component_count += 1;
                population.nested_membership_tests = population
                    .nested_membership_tests
                    .saturating_add(evidence.nested_membership_tests);
                population.event_push_count = population
                    .event_push_count
                    .saturating_add(evidence.event_push_count);
                population.event_pop_count = population
                    .event_pop_count
                    .saturating_add(evidence.event_pop_count);
            }
            Err(differences) => {
                population.clean_component_count += 1;
                population.mismatch_count += 1;
                let minimized = minimize_failure_component(&component);
                failures.push(GapDifferentialFailure {
                    population: population.population.clone(),
                    instance_name: instance.name.clone(),
                    width: component.grid_width,
                    height: component.grid_height,
                    cells: component_cells(&component),
                    minimized_width: minimized.grid_width,
                    minimized_height: minimized.grid_height,
                    minimized_cells: component_cells(&minimized),
                    differences,
                });
            }
        }
    }
}

fn component_cells(component: &GridComponent<bool>) -> Vec<bool> {
    let mut cells = vec![false; component.grid_width * component.grid_height];
    for cell in &component.cells {
        cells[cell.y * component.grid_width + cell.x] = true;
    }
    cells
}

fn minimize_failure_component(component: &GridComponent<bool>) -> GridComponent<bool> {
    let mut current = component.clone();
    let mut index = 0;
    while index < current.cells.len() {
        let mut cells = current.cells.clone();
        cells.remove(index);
        if cells.is_empty() {
            break;
        }
        let candidate = GridComponent {
            id: current.id,
            color: true,
            grid_width: current.grid_width,
            grid_height: current.grid_height,
            cells,
        };
        let connected = ColorGrid::new(
            candidate.grid_width,
            candidate.grid_height,
            component_cells(&candidate),
        )
        .ok()
        .and_then(|grid| {
            grid.four_connected_components()
                .into_iter()
                .find(|part| part.color && part.cell_count() == candidate.cell_count())
        });
        if let Some(candidate) = connected
            && compare_clean_component(&candidate).is_err()
        {
            current = candidate;
            index = 0;
            continue;
        }
        index += 1;
    }
    current
}

fn dihedral_transforms() -> [GridTransform; 8] {
    [
        GridTransform::ReflectHorizontal,
        GridTransform::ReflectVertical,
        GridTransform::Rotate90,
        GridTransform::Rotate180,
        GridTransform::Rotate270,
        GridTransform::ReflectMainDiagonal,
        GridTransform::ReflectAntiDiagonal,
        GridTransform::Translate { dx: 3, dy: 2 },
    ]
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
    let target_steps = width.saturating_mul(height).saturating_mul(3).max(1);
    for _ in 0..target_steps {
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
        name: format!("gap-random-connected-{case:06}"),
        family: "gap-random-connected".to_owned(),
        width,
        height,
        cells,
        parameters: [("case".to_owned(), case)].into_iter().collect(),
    }
}

/// Runs the complete configured `ReferenceNested` versus `EventSweep` population.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn verify_gap_backends(
    context: BenchmarkContext,
    config: GapDifferentialConfig,
) -> GapDifferentialReport {
    let mut populations = Vec::new();
    let mut failures = Vec::new();

    for &side in &config.exhaustive_sides {
        let mut population = GapDifferentialPopulation {
            population: format!("clean-binary-{side}x{side}"),
            ..GapDifferentialPopulation::default()
        };
        let bit_count = side.saturating_mul(side);
        if bit_count <= 20 {
            for mask in 1_u64..(1_u64 << bit_count) {
                evaluate_instance(
                    &mut population,
                    &mut failures,
                    &AdversarialInstance {
                        name: format!("binary-{side}x{side}-{mask:x}"),
                        family: "binary-clean-candidate".to_owned(),
                        width: side,
                        height: side,
                        cells: (0..bit_count)
                            .map(|index| mask & (1_u64 << index) != 0)
                            .collect(),
                        parameters: BTreeMap::new(),
                    },
                );
            }
        }
        populations.push(population);
    }

    let mut polyomino_population = GapDifferentialPopulation {
        population: format!("free-polyomino-through-{}", config.polyomino_max_cells),
        ..GapDifferentialPopulation::default()
    };
    for level in enumerate_free_polyominoes(config.polyomino_max_cells) {
        for polyomino in level {
            let instance = polyomino.to_instance(
                format!("gap-polyomino-{}", polyomino.canonical_key()),
                "free-polyomino",
            );
            evaluate_instance(&mut polyomino_population, &mut failures, &instance);
            if config.include_dihedral_transforms
                && let Ok(grid) =
                    ColorGrid::new(instance.width, instance.height, instance.cells.clone())
                && let Some(component) = grid
                    .four_connected_components()
                    .into_iter()
                    .find(|component| component.color)
            {
                for (index, transform) in dihedral_transforms().into_iter().enumerate() {
                    if let Ok(transformed) = TransformedComponent::new(&component, transform) {
                        evaluate_instance(
                            &mut polyomino_population,
                            &mut failures,
                            &AdversarialInstance {
                                name: format!("{}-transform-{index}", instance.name),
                                family: "free-polyomino-transform".to_owned(),
                                width: transformed.component.grid_width,
                                height: transformed.component.grid_height,
                                cells: component_cells(&transformed.component),
                                parameters: BTreeMap::new(),
                            },
                        );
                    }
                }
            }
        }
    }
    populations.push(polyomino_population);

    let mut random_population = GapDifferentialPopulation {
        population: "deterministic-connected-regions".to_owned(),
        ..GapDifferentialPopulation::default()
    };
    let mut random = SplitMix64::new(config.random_seed);
    for case in 0..config.random_cases {
        let instance =
            random_connected_instance(config.random_width, config.random_height, &mut random, case);
        evaluate_instance(&mut random_population, &mut failures, &instance);
    }
    populations.push(random_population);

    let mut family_population = GapDifferentialPopulation {
        population: "path-tree-geometry-families".to_owned(),
        ..GapDifferentialPopulation::default()
    };
    for &scale in &config.family_scales {
        for instance in path_tree_geometry_families(scale) {
            evaluate_instance(&mut family_population, &mut failures, &instance);
        }
    }
    for instance in topological_stress_instances() {
        evaluate_instance(&mut family_population, &mut failures, &instance);
    }
    for instance in crate::witness::stored_mixed_branching_witnesses() {
        evaluate_instance(&mut family_population, &mut failures, &instance);
    }
    populations.push(family_population);

    let mut complete_bipartite_population = GapDifferentialPopulation {
        population: format!(
            "complete-bipartite-through-{}",
            config.complete_bipartite_max_t
        ),
        ..GapDifferentialPopulation::default()
    };
    for t in 1..=config.complete_bipartite_max_t {
        if let Ok(instance) = clean_complete_bipartite_grid(t) {
            evaluate_instance(&mut complete_bipartite_population, &mut failures, &instance);
        }
    }
    populations.push(complete_bipartite_population);

    let total_input_count = populations.iter().map(|row| row.input_count).sum();
    let total_component_count = populations.iter().map(|row| row.component_count).sum();
    let total_clean_component_count = populations
        .iter()
        .map(|row| row.clean_component_count)
        .sum();
    let total_boundary_index_comparison_count = populations
        .iter()
        .map(|row| row.boundary_index_comparison_count)
        .sum();
    let total_endpoint_metadata_comparison_count = populations
        .iter()
        .map(|row| row.endpoint_metadata_comparison_count)
        .sum();
    let total_clean_classifier_comparison_count = populations
        .iter()
        .map(|row| row.clean_classifier_comparison_count)
        .sum();
    let total_orientation_comparison_count = populations
        .iter()
        .map(|row| row.orientation_comparison_count)
        .sum();
    let total_verified_component_count = populations
        .iter()
        .map(|row| row.verified_component_count)
        .sum();
    let total_mismatch_count = populations.iter().map(|row| row.mismatch_count).sum();
    let total_solver_error_count = populations.iter().map(|row| row.solver_error_count).sum();
    let total_nested_membership_tests = populations
        .iter()
        .map(|row| row.nested_membership_tests)
        .sum();
    let total_event_push_count = populations.iter().map(|row| row.event_push_count).sum();
    let total_event_pop_count = populations.iter().map(|row| row.event_pop_count).sum();
    let metadata = BenchmarkMetadata {
        git_commit: context.git_commit,
        rustc_version: context.rustc_version,
        command: context.command,
        seed: Some(config.random_seed),
        timestamp: context.timestamp,
        input_count: total_input_count,
        component_count: total_component_count,
        input_model: "finite-grid-reference-nested-vs-event-sweep-complete-population".to_owned(),
        unsupported_input_features: vec![
            "ornaments".to_owned(),
            "isolated-formal-boundary-points".to_owned(),
            "line-segment-holes".to_owned(),
            "point-holes".to_owned(),
            "degenerate-formal-holes".to_owned(),
            "general-polygon-input".to_owned(),
        ],
    };
    GapDifferentialReport {
        metadata,
        config,
        populations,
        total_input_count,
        total_component_count,
        total_clean_component_count,
        total_boundary_index_comparison_count,
        total_endpoint_metadata_comparison_count,
        total_clean_classifier_comparison_count,
        total_orientation_comparison_count,
        total_verified_component_count,
        total_mismatch_count,
        total_solver_error_count,
        total_nested_membership_tests,
        total_event_push_count,
        total_event_pop_count,
        failures,
    }
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
    use super::{GapDifferentialConfig, verify_gap_backends};
    use crate::benchmark::BenchmarkContext;

    #[test]
    fn gap_backends_match_on_bounded_permanent_population() {
        let report = verify_gap_backends(
            BenchmarkContext {
                git_commit: "test".to_owned(),
                rustc_version: "test".to_owned(),
                command: "test".to_owned(),
                seed: Some(42),
                timestamp: 0,
            },
            GapDifferentialConfig {
                exhaustive_sides: vec![3],
                polyomino_max_cells: 6,
                random_width: 8,
                random_height: 8,
                random_cases: 256,
                random_seed: 42,
                complete_bipartite_max_t: 8,
                family_scales: vec![3, 8],
                include_dihedral_transforms: true,
            },
        );
        assert!(report.verified(), "{:?}", report.failures);
        assert!(report.total_clean_component_count > 0);
        assert_eq!(report.total_event_push_count, report.total_event_pop_count);
    }
}
