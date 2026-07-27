use rect_core::{
    FormalEffectiveChordFamilies, FormalPolygonError, FormalRectilinearPolygon, HorizontalChord,
    VerticalChord, closed_chords_intersect,
};
use rect_graph::{
    BipartiteGraph, DinicBackend, Matching, VertexCover, hopcroft_karp, minimum_vertex_cover,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::biclique::{BicliqueError, BicliquePartition};
use crate::compressed_flow::{CompressedFlowError, solve_biclique_flow};
use crate::embedding::{DominanceEmbedding, EmbeddingError};

/// One exact integer-scaled segment in the source Step 2 family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalStep2Segment {
    pub first: [i128; 2],
    pub second: [i128; 2],
}

/// Exact symbolic certificate for Section 10 Step 2 (pp. 77--78).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalStep2Transformation {
    /// Common positive denominator for every transformed coordinate.
    pub coordinate_scale: i128,
    /// Numerator of epsilon under `coordinate_scale`.
    pub epsilon_numerator: i128,
    pub horizontal: Vec<FormalStep2Segment>,
    pub vertical: Vec<FormalStep2Segment>,
    pub original_intersection_count: usize,
    pub transformed_intersection_count: usize,
    pub original_collinear_contact_count: usize,
}

/// Complete P3.3 reduction retaining the explicit and compact Oracles.
#[derive(Clone, Debug)]
pub struct FormalAdmissibleAnalysis {
    pub families: FormalEffectiveChordFamilies,
    pub transformation: FormalStep2Transformation,
    pub explicit_conflict_graph: BipartiteGraph,
    pub explicit_matching: Matching,
    pub explicit_vertex_cover: VertexCover,
    pub compact_vertex_cover: VertexCover,
    pub selected_horizontal: Vec<bool>,
    pub selected_vertical: Vec<bool>,
    pub effective_number: usize,
    pub local_nonconvexity_measure: usize,
    pub interior_component_count: usize,
    pub formal_hole_count: usize,
    pub optimum_rectangle_count: usize,
}

struct ConflictSolutions {
    graph: BipartiteGraph,
    matching: Matching,
    vertex_cover: VertexCover,
}

/// Solves the maximum formal admissible family through independent explicit
/// and compact conflict representations.
///
/// The Step 2 transformation is retained as a checked certificate. Matching
/// remains indexed by the original effective chords, because the source
/// transformation is one-to-one and preserves every orthogonal intersection.
///
/// # Errors
///
/// Returns an error for formal geometry failures, checked-coordinate overflow,
/// a failed transformation invariant, or any matching/cut disagreement.
pub fn analyze_formal_admissible_family(
    polygon: &FormalRectilinearPolygon,
) -> Result<FormalAdmissibleAnalysis, FormalAdmissibleError> {
    let families = polygon.effective_chords_source()?.families;
    let transformation = FormalStep2Transformation::new(&families)?;
    let ConflictSolutions {
        graph: explicit_conflict_graph,
        matching: explicit_matching,
        vertex_cover: compact_vertex_cover,
    } = solve_conflict_oracles(&families)?;

    let selected_horizontal = compact_vertex_cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = compact_vertex_cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    assert_independent_family(
        &explicit_conflict_graph,
        &selected_horizontal,
        &selected_vertical,
    )?;
    let chord_count = families.horizontal.len() + families.vertical.len();
    let effective_number = chord_count
        .checked_sub(explicit_matching.size)
        .ok_or(FormalAdmissibleError::FormulaUnderflow)?;
    let selected_count = selected_horizontal
        .iter()
        .filter(|&&selected| selected)
        .count()
        + selected_vertical
            .iter()
            .filter(|&&selected| selected)
            .count();
    if selected_count != effective_number {
        return Err(FormalAdmissibleError::IndependentFamilySizeMismatch {
            selected: selected_count,
            expected: effective_number,
        });
    }

    let local_nonconvexity_measure = polygon
        .vertex_geometry()?
        .iter()
        .try_fold(0_usize, |sum, vertex| {
            sum.checked_add(usize::from(vertex.local_nonconvexity_measure))
        });
    let local_nonconvexity_measure =
        local_nonconvexity_measure.ok_or(FormalAdmissibleError::FormulaOverflow)?;
    let formal_hole_count = polygon.incidence()?.formal_holes().count();
    // RectilinearPolygon represents one connected ordinary interior component.
    let interior_component_count = 1_usize;
    let optimum_rectangle_count = local_nonconvexity_measure
        .checked_add(interior_component_count)
        .and_then(|value| value.checked_sub(formal_hole_count))
        .and_then(|value| value.checked_sub(effective_number))
        .ok_or(FormalAdmissibleError::FormulaUnderflow)?;

    Ok(FormalAdmissibleAnalysis {
        families,
        transformation,
        explicit_conflict_graph,
        explicit_matching,
        explicit_vertex_cover: compact_vertex_cover.clone(),
        compact_vertex_cover,
        selected_horizontal,
        selected_vertical,
        effective_number,
        local_nonconvexity_measure,
        interior_component_count,
        formal_hole_count,
        optimum_rectangle_count,
    })
}

fn solve_conflict_oracles(
    families: &FormalEffectiveChordFamilies,
) -> Result<ConflictSolutions, FormalAdmissibleError> {
    let embedding = DominanceEmbedding::new(&families.horizontal, &families.vertical)?;
    embedding.assert_pairwise_equivalence(&families.horizontal, &families.vertical)?;
    let graph = embedding.explicit_graph()?;
    let matching = hopcroft_karp(&graph);
    let vertex_cover = minimum_vertex_cover(&graph, &matching);

    let explicit_partition = BicliquePartition::from_explicit_edges(&graph);
    let explicit_flow = solve_biclique_flow(
        families.horizontal.len(),
        families.vertical.len(),
        &explicit_partition,
        &DinicBackend,
    )?;
    let compact_partition = BicliquePartition::comparability_theorem_8(&embedding)?;
    compact_partition.verify_exact_partition(&graph)?;
    compact_partition.verify_dominance_blocks(&embedding)?;
    let compact_flow = solve_biclique_flow(
        families.horizontal.len(),
        families.vertical.len(),
        &compact_partition,
        &DinicBackend,
    )?;
    let explicit_flow_value = usize::try_from(explicit_flow.flow.value)
        .map_err(|_| FormalAdmissibleError::FlowValueConversion)?;
    let compact_flow_value = usize::try_from(compact_flow.flow.value)
        .map_err(|_| FormalAdmissibleError::FlowValueConversion)?;
    if matching.size != explicit_flow_value || explicit_flow_value != compact_flow_value {
        return Err(FormalAdmissibleError::MatchingValueMismatch {
            matching: matching.size,
            explicit_flow: explicit_flow_value,
            compact_flow: compact_flow_value,
        });
    }
    if vertex_cover != explicit_flow.vertex_cover || vertex_cover != compact_flow.vertex_cover {
        return Err(FormalAdmissibleError::VertexCoverMismatch);
    }
    Ok(ConflictSolutions {
        graph,
        matching,
        vertex_cover,
    })
}

impl FormalStep2Transformation {
    fn new(families: &FormalEffectiveChordFamilies) -> Result<Self, FormalAdmissibleError> {
        let scale_base = families
            .horizontal
            .len()
            .checked_add(families.vertical.len())
            .and_then(|value| value.checked_add(1))
            .ok_or(FormalAdmissibleError::TransformationOverflow)?;
        let scale_base = i128::try_from(scale_base)
            .map_err(|_| FormalAdmissibleError::TransformationOverflow)?;
        // epsilon = 1/4 and delta_i = i / (4 * (p + q + 1)).
        let coordinate_scale = scale_base
            .checked_mul(4)
            .ok_or(FormalAdmissibleError::TransformationOverflow)?;
        let epsilon_numerator = scale_base;
        let horizontal = families
            .horizontal
            .iter()
            .enumerate()
            .map(|(index, chord)| {
                transform_horizontal(*chord, index, coordinate_scale, epsilon_numerator)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let vertical = families
            .vertical
            .iter()
            .enumerate()
            .map(|(index, chord)| {
                transform_vertical(*chord, index, coordinate_scale, epsilon_numerator)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut original_intersection_count = 0;
        let mut transformed_intersection_count = 0;
        for (horizontal_index, horizontal_chord) in families.horizontal.iter().enumerate() {
            for (vertical_index, vertical_chord) in families.vertical.iter().enumerate() {
                let original = closed_chords_intersect(*horizontal_chord, *vertical_chord);
                let transformed =
                    transformed_intersect(horizontal[horizontal_index], vertical[vertical_index]);
                original_intersection_count += usize::from(original);
                transformed_intersection_count += usize::from(transformed);
                if original != transformed {
                    return Err(FormalAdmissibleError::TransformationIntersectionMismatch {
                        horizontal: horizontal_index,
                        vertical: vertical_index,
                    });
                }
            }
        }
        if horizontal_segments_intersect(&horizontal) || vertical_segments_intersect(&vertical) {
            return Err(FormalAdmissibleError::TransformedCollinearContact);
        }
        Ok(Self {
            coordinate_scale,
            epsilon_numerator,
            horizontal,
            vertical,
            original_intersection_count,
            transformed_intersection_count,
            original_collinear_contact_count: count_collinear_contacts(
                &families.horizontal,
                &families.vertical,
            ),
        })
    }
}

fn assert_independent_family(
    graph: &BipartiteGraph,
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
) -> Result<(), FormalAdmissibleError> {
    for (left, right) in graph.edges() {
        if selected_horizontal[left] && selected_vertical[right] {
            return Err(FormalAdmissibleError::SelectedConflict { left, right });
        }
    }
    Ok(())
}

fn scaled(value: i64, scale: i128) -> Result<i128, FormalAdmissibleError> {
    i128::from(value)
        .checked_mul(scale)
        .ok_or(FormalAdmissibleError::TransformationOverflow)
}

fn ordinal(index: usize) -> Result<i128, FormalAdmissibleError> {
    i128::try_from(index)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(FormalAdmissibleError::TransformationOverflow)
}

fn transform_horizontal(
    chord: HorizontalChord,
    index: usize,
    scale: i128,
    epsilon: i128,
) -> Result<FormalStep2Segment, FormalAdmissibleError> {
    let delta = ordinal(index)?;
    let y = scaled(chord.y(), scale)?
        .checked_add(delta)
        .ok_or(FormalAdmissibleError::TransformationOverflow)?;
    Ok(FormalStep2Segment {
        first: [
            scaled(chord.left(), scale)?
                .checked_sub(epsilon)
                .ok_or(FormalAdmissibleError::TransformationOverflow)?,
            y,
        ],
        second: [
            scaled(chord.right(), scale)?
                .checked_add(epsilon)
                .ok_or(FormalAdmissibleError::TransformationOverflow)?,
            y,
        ],
    })
}

fn transform_vertical(
    chord: VerticalChord,
    index: usize,
    scale: i128,
    epsilon: i128,
) -> Result<FormalStep2Segment, FormalAdmissibleError> {
    let delta = ordinal(index)?;
    let x = scaled(chord.x(), scale)?
        .checked_add(delta)
        .ok_or(FormalAdmissibleError::TransformationOverflow)?;
    Ok(FormalStep2Segment {
        first: [
            x,
            scaled(chord.bottom(), scale)?
                .checked_sub(epsilon)
                .ok_or(FormalAdmissibleError::TransformationOverflow)?,
        ],
        second: [
            x,
            scaled(chord.top(), scale)?
                .checked_add(epsilon)
                .ok_or(FormalAdmissibleError::TransformationOverflow)?,
        ],
    })
}

fn transformed_intersect(horizontal: FormalStep2Segment, vertical: FormalStep2Segment) -> bool {
    horizontal.first[0] <= vertical.first[0]
        && vertical.first[0] <= horizontal.second[0]
        && vertical.first[1] <= horizontal.first[1]
        && horizontal.first[1] <= vertical.second[1]
}

fn horizontal_segments_intersect(segments: &[FormalStep2Segment]) -> bool {
    segments.iter().enumerate().any(|(index, first)| {
        segments.iter().skip(index + 1).any(|second| {
            first.first[1] == second.first[1]
                && first.first[0] <= second.second[0]
                && second.first[0] <= first.second[0]
        })
    })
}

fn vertical_segments_intersect(segments: &[FormalStep2Segment]) -> bool {
    segments.iter().enumerate().any(|(index, first)| {
        segments.iter().skip(index + 1).any(|second| {
            first.first[0] == second.first[0]
                && first.first[1] <= second.second[1]
                && second.first[1] <= first.second[1]
        })
    })
}

fn count_collinear_contacts(horizontal: &[HorizontalChord], vertical: &[VerticalChord]) -> usize {
    let horizontal_contacts = horizontal
        .iter()
        .enumerate()
        .map(|(index, first)| {
            horizontal
                .iter()
                .skip(index + 1)
                .filter(|second| {
                    first.y() == second.y()
                        && first.left() <= second.right()
                        && second.left() <= first.right()
                })
                .count()
        })
        .sum::<usize>();
    let vertical_contacts = vertical
        .iter()
        .enumerate()
        .map(|(index, first)| {
            vertical
                .iter()
                .skip(index + 1)
                .filter(|second| {
                    first.x() == second.x()
                        && first.bottom() <= second.top()
                        && second.bottom() <= first.top()
                })
                .count()
        })
        .sum::<usize>();
    horizontal_contacts + vertical_contacts
}

#[derive(Debug, Error)]
pub enum FormalAdmissibleError {
    #[error(transparent)]
    FormalPolygon(#[from] FormalPolygonError),
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    #[error(transparent)]
    Biclique(#[from] BicliqueError),
    #[error(transparent)]
    CompressedFlow(#[from] CompressedFlowError),
    #[error("Step 2 exact-coordinate transformation overflowed")]
    TransformationOverflow,
    #[error("Step 2 changed intersection pair ({horizontal}, {vertical})")]
    TransformationIntersectionMismatch { horizontal: usize, vertical: usize },
    #[error("two transformed collinear segments retain a common point")]
    TransformedCollinearContact,
    #[error("flow value cannot be represented as usize")]
    FlowValueConversion,
    #[error(
        "matching values disagree: Hopcroft--Karp={matching}, explicit flow={explicit_flow}, compact flow={compact_flow}"
    )]
    MatchingValueMismatch {
        matching: usize,
        explicit_flow: usize,
        compact_flow: usize,
    },
    #[error("explicit and compact minimum vertex covers disagree")]
    VertexCoverMismatch,
    #[error("selected family contains conflict edge ({left}, {right})")]
    SelectedConflict { left: usize, right: usize },
    #[error("selected family size {selected} differs from expected maximum {expected}")]
    IndependentFamilySizeMismatch { selected: usize, expected: usize },
    #[error("formal optimum formula overflowed")]
    FormulaOverflow,
    #[error("formal optimum formula underflowed")]
    FormulaUnderflow,
}

#[cfg(test)]
mod tests {
    use rect_core::{
        FormalRectilinearPolygon, Ornament, OrnamentSegment, OrthogonalLoop, Point,
        RectilinearPolygon,
    };

    use super::analyze_formal_admissible_family;

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    fn source_figure_three() -> FormalRectilinearPolygon {
        FormalRectilinearPolygon::new(
            RectilinearPolygon::new(rectangle(0, 0, 12, 12), vec![rectangle(2, 6, 5, 9)]).unwrap(),
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

    #[test]
    fn source_figure_three_has_identical_explicit_and_compact_certificates() {
        let analysis = analyze_formal_admissible_family(&source_figure_three()).unwrap();
        assert_eq!(analysis.families.horizontal.len(), 4);
        assert_eq!(analysis.families.vertical.len(), 2);
        assert_eq!(analysis.explicit_matching.size, 2);
        assert_eq!(analysis.effective_number, 4);
        assert_eq!(
            analysis.explicit_vertex_cover,
            analysis.compact_vertex_cover
        );
        assert_eq!(
            analysis.transformation.original_intersection_count,
            analysis.transformation.transformed_intersection_count
        );
    }

    #[test]
    fn step_two_separates_collinear_chords_at_an_isolated_endpoint() {
        let polygon = FormalRectilinearPolygon::new(
            RectilinearPolygon::new(rectangle(0, 0, 12, 12), vec![]).unwrap(),
            Ornament {
                isolated_points: vec![Point::new(2, 6), Point::new(6, 6), Point::new(10, 6)],
                segments: vec![],
            },
        )
        .unwrap();
        let analysis = analyze_formal_admissible_family(&polygon).unwrap();
        assert_eq!(analysis.families.horizontal.len(), 2);
        assert_eq!(analysis.transformation.original_collinear_contact_count, 1);
        assert_ne!(
            analysis.transformation.horizontal[0].first[1],
            analysis.transformation.horizontal[1].first[1]
        );
        assert_eq!(analysis.effective_number, 2);
        assert_eq!(analysis.local_nonconvexity_measure, 6);
        assert_eq!(analysis.interior_component_count, 1);
        assert_eq!(analysis.formal_hole_count, 3);
        assert_eq!(analysis.optimum_rectangle_count, 2);
    }

    #[test]
    fn empty_ornament_uses_the_ordinary_optimum_formula() {
        let polygon = FormalRectilinearPolygon::new(
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(8, 0),
                    Point::new(8, 3),
                    Point::new(5, 3),
                    Point::new(5, 6),
                    Point::new(3, 6),
                    Point::new(3, 3),
                    Point::new(0, 3),
                ]),
                vec![],
            )
            .unwrap(),
            Ornament::default(),
        )
        .unwrap();
        let analysis = analyze_formal_admissible_family(&polygon).unwrap();
        assert_eq!(analysis.formal_hole_count, 0);
        assert_eq!(analysis.local_nonconvexity_measure, 2);
        assert_eq!(analysis.effective_number, 1);
        assert_eq!(analysis.optimum_rectangle_count, 2);
    }

    #[test]
    fn isolated_point_lattice_preserves_every_backend_certificate() {
        let points = [
            Point::new(3, 3),
            Point::new(6, 3),
            Point::new(9, 3),
            Point::new(3, 6),
            Point::new(6, 6),
            Point::new(9, 6),
            Point::new(3, 9),
            Point::new(6, 9),
            Point::new(9, 9),
        ];
        for mask in 1_u16..1 << points.len() {
            let polygon = FormalRectilinearPolygon::new(
                RectilinearPolygon::new(rectangle(0, 0, 12, 12), vec![]).unwrap(),
                Ornament {
                    isolated_points: points
                        .iter()
                        .enumerate()
                        .filter_map(|(index, &point)| (mask & (1 << index) != 0).then_some(point))
                        .collect(),
                    segments: vec![],
                },
            )
            .unwrap();
            let analysis = analyze_formal_admissible_family(&polygon).unwrap();
            assert_eq!(
                analysis.explicit_vertex_cover, analysis.compact_vertex_cover,
                "mask {mask}"
            );
            assert_eq!(
                analysis.transformation.original_intersection_count,
                analysis.transformation.transformed_intersection_count,
                "mask {mask}"
            );
        }
    }
}
