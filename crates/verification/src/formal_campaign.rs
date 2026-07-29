//! Reproducible formal-boundary fixture and ordinary-parity campaign.

use dominance::experiment::{
    PolygonSolveOptions, Verification, complete_formal_polygon, solve_polygon_with_options,
};
use mrd_domain::{
    FormalRectilinearPolygon, Ornament, OrnamentSegment, OrthogonalLoop, Point, RectilinearPolygon,
};
use serde::{Deserialize, Serialize};

use crate::benchmark::{BenchmarkContext, BenchmarkMetadata};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalFixtureRecord {
    pub name: String,
    pub local_nonconvexity_measure: usize,
    pub interior_component_count: usize,
    pub formal_hole_count: usize,
    pub effective_number: usize,
    pub optimum_rectangle_count: usize,
    pub rectangle_count: usize,
    pub certificates_equal: bool,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OrdinaryFormalParityRecord {
    pub name: String,
    pub optimum_equal: bool,
    pub rectangles_equal: bool,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalCampaignReport {
    pub metadata: BenchmarkMetadata,
    pub fixture_records: Vec<FormalFixtureRecord>,
    pub ordinary_parity_records: Vec<OrdinaryFormalParityRecord>,
    pub verified: usize,
    pub solver_errors: usize,
    pub disagreements: usize,
}

impl FormalCampaignReport {
    #[must_use]
    pub const fn verified(&self) -> bool {
        self.solver_errors == 0 && self.disagreements == 0
    }
}

#[must_use]
pub fn formal_fixture_campaign(context: BenchmarkContext) -> FormalCampaignReport {
    let fixtures = formal_fixtures();
    let ordinary = ordinary_parity_fixtures();
    let fixture_records = fixtures
        .into_iter()
        .map(|(name, polygon)| formal_fixture_record(name, &polygon))
        .collect::<Vec<_>>();
    let ordinary_parity_records = ordinary
        .into_iter()
        .map(|(name, polygon)| ordinary_parity_record(name, &polygon))
        .collect::<Vec<_>>();
    let solver_errors = fixture_records
        .iter()
        .filter(|record| record.status == "solver-error")
        .count()
        + ordinary_parity_records
            .iter()
            .filter(|record| record.status == "solver-error")
            .count();
    let disagreements = fixture_records
        .iter()
        .filter(|record| record.status == "disagreement")
        .count()
        + ordinary_parity_records
            .iter()
            .filter(|record| record.status == "disagreement")
            .count();
    let verified =
        fixture_records.len() + ordinary_parity_records.len() - solver_errors - disagreements;

    FormalCampaignReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: fixture_records.len() + ordinary_parity_records.len(),
            component_count: fixture_records.len() + ordinary_parity_records.len(),
            input_model: "formal-rectilinear-polygon".to_owned(),
            unsupported_input_features: Vec::new(),
        },
        fixture_records,
        ordinary_parity_records,
        verified,
        solver_errors,
        disagreements,
    }
}

fn formal_fixture_record(name: &str, polygon: &FormalRectilinearPolygon) -> FormalFixtureRecord {
    let result = match complete_formal_polygon(polygon) {
        Ok(result) => result,
        Err(error) => {
            return FormalFixtureRecord {
                name: name.to_owned(),
                local_nonconvexity_measure: 0,
                interior_component_count: 0,
                formal_hole_count: 0,
                effective_number: 0,
                optimum_rectangle_count: 0,
                rectangle_count: 0,
                certificates_equal: false,
                status: "solver-error".to_owned(),
                message: Some(error.to_string()),
            };
        }
    };
    let admissible = result.admissible;
    let rectangle_count = result.completion.rectangles.len();
    let certificates_equal = admissible.explicit_vertex_cover == admissible.compact_vertex_cover;
    let formula = admissible
        .local_nonconvexity_measure
        .checked_add(admissible.interior_component_count)
        .and_then(|value| value.checked_sub(admissible.formal_hole_count))
        .and_then(|value| value.checked_sub(admissible.effective_number));
    let equal = formula == Some(admissible.optimum_rectangle_count)
        && rectangle_count == admissible.optimum_rectangle_count
        && certificates_equal;
    FormalFixtureRecord {
        name: name.to_owned(),
        local_nonconvexity_measure: admissible.local_nonconvexity_measure,
        interior_component_count: admissible.interior_component_count,
        formal_hole_count: admissible.formal_hole_count,
        effective_number: admissible.effective_number,
        optimum_rectangle_count: admissible.optimum_rectangle_count,
        rectangle_count,
        certificates_equal,
        status: if equal { "verified" } else { "disagreement" }.to_owned(),
        message: (!equal)
            .then_some("formal formula, certificate, or rectangle count disagreement".to_owned()),
    }
}

fn ordinary_parity_record(name: &str, polygon: &RectilinearPolygon) -> OrdinaryFormalParityRecord {
    let formal = match FormalRectilinearPolygon::new(polygon.clone(), Ornament::default()) {
        Ok(formal) => formal,
        Err(error) => return parity_error(name, error.to_string()),
    };
    let ordinary = match solve_polygon_with_options(
        polygon,
        PolygonSolveOptions {
            verification_mode: Verification::FullyAudited,
            ..PolygonSolveOptions::default()
        },
    ) {
        Ok(ordinary) => ordinary,
        Err(error) => return parity_error(name, error.to_string()),
    };
    let formal = match complete_formal_polygon(&formal) {
        Ok(formal) => formal,
        Err(error) => return parity_error(name, error.to_string()),
    };
    let optimum_equal =
        formal.admissible.optimum_rectangle_count == ordinary.optimum_rectangle_count;
    let rectangles_equal = formal.completion.rectangles == ordinary.rectangles;
    let equal = optimum_equal && rectangles_equal;
    OrdinaryFormalParityRecord {
        name: name.to_owned(),
        optimum_equal,
        rectangles_equal,
        status: if equal { "verified" } else { "disagreement" }.to_owned(),
        message: (!equal)
            .then_some("empty-ornament formal result differs from ordinary solver".to_owned()),
    }
}

fn parity_error(name: &str, message: String) -> OrdinaryFormalParityRecord {
    OrdinaryFormalParityRecord {
        name: name.to_owned(),
        optimum_equal: false,
        rectangles_equal: false,
        status: "solver-error".to_owned(),
        message: Some(message),
    }
}

fn formal_fixtures() -> Vec<(&'static str, FormalRectilinearPolygon)> {
    let rectangle = || RectilinearPolygon::new(rectangle_loop(0, 0, 12, 12), vec![]).unwrap();
    vec![
        (
            "point-hole",
            FormalRectilinearPolygon::new(
                rectangle(),
                Ornament {
                    isolated_points: vec![Point::new(6, 6)],
                    segments: vec![],
                },
            )
            .unwrap(),
        ),
        (
            "segment-hole",
            FormalRectilinearPolygon::new(
                rectangle(),
                Ornament {
                    isolated_points: vec![],
                    segments: vec![
                        OrnamentSegment::new(Point::new(3, 6), Point::new(9, 6)).unwrap(),
                    ],
                },
            )
            .unwrap(),
        ),
        (
            "attached-hole",
            FormalRectilinearPolygon::new(
                rectangle(),
                Ornament {
                    isolated_points: vec![],
                    segments: vec![
                        OrnamentSegment::new(Point::new(0, 6), Point::new(7, 6)).unwrap(),
                    ],
                },
            )
            .unwrap(),
        ),
        (
            "shared-endpoint",
            FormalRectilinearPolygon::new(
                rectangle(),
                Ornament {
                    isolated_points: vec![Point::new(2, 6), Point::new(6, 6), Point::new(10, 6)],
                    segments: vec![],
                },
            )
            .unwrap(),
        ),
        ("source-figure-three", source_figure_three()),
    ]
}

fn ordinary_parity_fixtures() -> Vec<(&'static str, RectilinearPolygon)> {
    vec![
        (
            "rectangle",
            RectilinearPolygon::new(rectangle_loop(0, 0, 12, 12), vec![]).unwrap(),
        ),
        (
            "l-shape",
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(8, 0),
                    Point::new(8, 3),
                    Point::new(3, 3),
                    Point::new(3, 8),
                    Point::new(0, 8),
                ]),
                vec![],
            )
            .unwrap(),
        ),
        (
            "ordinary-hole",
            RectilinearPolygon::new(
                rectangle_loop(0, 0, 12, 12),
                vec![rectangle_loop(3, 3, 6, 6)],
            )
            .unwrap(),
        ),
    ]
}

fn source_figure_three() -> FormalRectilinearPolygon {
    FormalRectilinearPolygon::new(
        RectilinearPolygon::new(
            rectangle_loop(0, 0, 12, 12),
            vec![rectangle_loop(2, 6, 5, 9)],
        )
        .unwrap(),
        Ornament {
            isolated_points: vec![Point::new(6, 3), Point::new(6, 9), Point::new(8, 9)],
            segments: vec![
                OrnamentSegment::new(Point::new(10, 0), Point::new(10, 3)).unwrap(),
                OrnamentSegment::new(Point::new(2, 3), Point::new(5, 3)).unwrap(),
                OrnamentSegment::new(Point::new(10, 6), Point::new(12, 6)).unwrap(),
                OrnamentSegment::new(Point::new(10, 9), Point::new(10, 12)).unwrap(),
            ],
        },
    )
    .unwrap()
}

fn rectangle_loop(left: i64, bottom: i64, right: i64, top: i64) -> OrthogonalLoop {
    OrthogonalLoop::new(vec![
        Point::new(left, bottom),
        Point::new(right, bottom),
        Point::new(right, top),
        Point::new(left, top),
    ])
}

#[cfg(test)]
mod tests {
    use super::{BenchmarkContext, formal_fixture_campaign};

    #[test]
    fn formal_fixtures_and_empty_ornament_parity_are_exact() {
        let report = formal_fixture_campaign(BenchmarkContext {
            git_commit: "test".to_owned(),
            rustc_version: "test".to_owned(),
            command: "test".to_owned(),
            seed: None,
            timestamp: 0,
        });
        assert!(report.verified(), "{report:#?}");
        assert_eq!(report.fixture_records.len(), 5);
        assert_eq!(report.ordinary_parity_records.len(), 3);
    }
}
