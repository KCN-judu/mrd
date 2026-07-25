use std::collections::BTreeMap;

use rect_core::{
    ColorGrid, Diagnostics, DissectionResult, GridComponent, GridRect, ValidationError,
    validate_dissection,
};
use rect_dominance::{DominanceMode, VerificationMode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalOracleResult {
    pub schema_version: usize,
    pub solver: String,
    pub status: String,
    pub input_hash: String,
    pub runtime_seconds: f64,
    pub components: Vec<ExternalComponentResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalComponentResult {
    pub component_id: usize,
    pub color: Value,
    pub cell_count: usize,
    pub status: String,
    pub optimum_rectangle_count: Option<usize>,
    pub rectangles: Vec<GridRect>,
    pub candidate_rectangle_count: usize,
    pub runtime_seconds_micros: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalComponentComparison {
    pub component_id: usize,
    pub external_status: String,
    pub external_optimum: Option<usize>,
    pub rust_optima: BTreeMap<String, usize>,
    pub rust_skipped_solvers: Vec<String>,
    pub external_rectangles_valid: bool,
    pub agrees: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalComparisonReport {
    pub input_hash: String,
    pub external_solver: String,
    pub component_count: usize,
    pub all_agree: bool,
    pub components: Vec<ExternalComponentComparison>,
}

/// Validates an external result and compares it against all Rust solvers.
///
/// # Errors
///
/// Returns [`ExternalComparisonError`] for a hash/schema/component mismatch,
/// invalid external geometry, or any Rust solver failure.
pub fn compare_external(
    grid: &ColorGrid<Value>,
    input_hash: &str,
    external: &ExternalOracleResult,
    exact_cover_cell_limit: usize,
) -> Result<ExternalComparisonReport, ExternalComparisonError> {
    if external.schema_version != 1 {
        return Err(ExternalComparisonError::SchemaVersion(
            external.schema_version,
        ));
    }
    if external.input_hash != input_hash {
        return Err(ExternalComparisonError::InputHash {
            expected: input_hash.to_owned(),
            actual: external.input_hash.clone(),
        });
    }
    let components = grid.four_connected_components();
    if components.len() != external.components.len() {
        return Err(ExternalComparisonError::ComponentCount {
            rust: components.len(),
            external: external.components.len(),
        });
    }
    let mut comparisons = Vec::with_capacity(components.len());
    for (component, external_component) in components.iter().zip(&external.components) {
        if component.id.0 != external_component.component_id
            || component.color != external_component.color
            || component.cell_count() != external_component.cell_count
        {
            return Err(ExternalComparisonError::ComponentIdentity {
                component: component.id.0,
            });
        }
        let external_optimum = external_component.optimum_rectangle_count;
        let external_rectangles_valid = if let Some(optimum) = external_optimum {
            let result = DissectionResult {
                optimum_rectangle_count: optimum,
                rectangles: external_component.rectangles.clone(),
                diagnostics: Diagnostics::default(),
                certificate: None,
            };
            validate_dissection(component, &result)?;
            true
        } else {
            false
        };
        let (rust_optima, rust_skipped_solvers) =
            collect_rust_optima(component, exact_cover_cell_limit)?;
        let agrees = external_component.status == "optimal"
            && external_optimum.is_some()
            && rust_optima
                .values()
                .all(|&value| Some(value) == external_optimum);
        comparisons.push(ExternalComponentComparison {
            component_id: component.id.0,
            external_status: external_component.status.clone(),
            external_optimum,
            rust_optima,
            rust_skipped_solvers,
            external_rectangles_valid,
            agrees,
        });
    }
    Ok(ExternalComparisonReport {
        input_hash: input_hash.to_owned(),
        external_solver: external.solver.clone(),
        component_count: comparisons.len(),
        all_agree: comparisons.iter().all(|comparison| comparison.agrees),
        components: comparisons,
    })
}

fn collect_rust_optima(
    component: &GridComponent<Value>,
    exact_cover_cell_limit: usize,
) -> Result<(BTreeMap<String, usize>, Vec<String>), ExternalComparisonError> {
    let mut rust_results = vec![
        (
            "sg-explicit",
            rect_oracle_sg::solve(component).map_err(|error| error.to_string()),
        ),
        (
            "dominance-c0",
            rect_dominance::solve(component, DominanceMode::ExplicitEdges)
                .map_err(|error| error.to_string()),
        ),
        (
            "dominance-compressed",
            rect_dominance::solve(component, DominanceMode::Compact)
                .map_err(|error| error.to_string()),
        ),
        (
            "dominance-compact-only",
            rect_dominance::solve_with_verification_mode(component, VerificationMode::CompactOnly)
                .map_err(|error| error.to_string()),
        ),
    ];
    let mut skipped = Vec::new();
    if component.cell_count() <= exact_cover_cell_limit {
        rust_results.push((
            "exact-cover",
            rect_oracle_exact_cover::solve(component).map_err(|error| error.to_string()),
        ));
    } else {
        skipped.push("exact-cover".to_owned());
    }
    let mut optima = BTreeMap::new();
    for (name, result) in rust_results {
        let result = result.map_err(|message| ExternalComparisonError::RustSolver {
            component: component.id.0,
            solver: name,
            message,
        })?;
        optima.insert(name.to_owned(), result.optimum_rectangle_count);
    }
    Ok((optima, skipped))
}

#[derive(Debug, Error)]
pub enum ExternalComparisonError {
    #[error("unsupported external result schema version {0}")]
    SchemaVersion(usize),
    #[error("input hash mismatch: expected {expected}, external result has {actual}")]
    InputHash { expected: String, actual: String },
    #[error("component count mismatch: Rust found {rust}, external oracle found {external}")]
    ComponentCount { rust: usize, external: usize },
    #[error("external component identity differs at component {component}")]
    ComponentIdentity { component: usize },
    #[error("external rectangle output is invalid: {0}")]
    InvalidExternalGeometry(#[from] ValidationError),
    #[error("component {component} Rust solver {solver} failed: {message}")]
    RustSolver {
        component: usize,
        solver: &'static str,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use rect_core::ColorGrid;
    use serde_json::json;

    use super::{ExternalComponentResult, ExternalOracleResult, compare_external};

    #[test]
    fn validates_and_compares_a_synthetic_external_result() {
        let grid = ColorGrid::new(2, 1, vec![json!("x"), json!("x")]).unwrap();
        let external = ExternalOracleResult {
            schema_version: 1,
            solver: "synthetic-test".to_owned(),
            status: "optimal".to_owned(),
            input_hash: "abc".to_owned(),
            runtime_seconds: 0.0,
            components: vec![ExternalComponentResult {
                component_id: 0,
                color: json!("x"),
                cell_count: 2,
                status: "optimal".to_owned(),
                optimum_rectangle_count: Some(1),
                rectangles: vec![rect_core::GridRect::new(0, 0, 2, 1).unwrap()],
                candidate_rectangle_count: 3,
                runtime_seconds_micros: 0,
            }],
        };
        let report = compare_external(&grid, "abc", &external, 40).unwrap();
        assert!(report.all_agree);
    }
}
