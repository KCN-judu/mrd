use std::collections::{BTreeSet, HashMap};

use rect_core::{Coord, HorizontalChord, VerticalChord, closed_chords_intersect};
use rect_graph::{BipartiteGraph, GraphError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct DominancePoint {
    pub coordinates: [i128; 4],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DominanceEmbedding {
    pub horizontal: Vec<DominancePoint>,
    pub vertical: Vec<DominancePoint>,
}

impl DominanceEmbedding {
    /// Builds the paper's exact even/odd endpoint-preserving rank encoding.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError`] if a rank is missing or checked arithmetic overflows.
    pub fn new(
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
    ) -> Result<Self, EmbeddingError> {
        let x_ranks = coordinate_ranks(
            horizontal_chords
                .iter()
                .flat_map(|chord| [chord.left(), chord.right()])
                .chain(vertical_chords.iter().map(|chord| chord.x())),
        );
        let y_ranks = coordinate_ranks(
            horizontal_chords.iter().map(|chord| chord.y()).chain(
                vertical_chords
                    .iter()
                    .flat_map(|chord| [chord.bottom(), chord.top()]),
            ),
        );

        let horizontal = horizontal_chords
            .iter()
            .map(|chord| alpha(*chord, &x_ranks, &y_ranks))
            .collect::<Result<Vec<_>, _>>()?;
        let vertical = vertical_chords
            .iter()
            .map(|chord| beta(*chord, &x_ranks, &y_ranks))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            horizontal,
            vertical,
        })
    }

    /// Exhaustively checks geometry/dominance equivalence for supplied families.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError`] for dimensions, any mismatched pair, or a
    /// forbidden cross-side coordinate equality.
    pub fn assert_pairwise_equivalence(
        &self,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
    ) -> Result<(), EmbeddingError> {
        if self.horizontal.len() != horizontal_chords.len()
            || self.vertical.len() != vertical_chords.len()
        {
            return Err(EmbeddingError::DimensionMismatch);
        }
        for (left, (&chord_h, &point_h)) in
            horizontal_chords.iter().zip(&self.horizontal).enumerate()
        {
            for (right, (&chord_v, &point_v)) in
                vertical_chords.iter().zip(&self.vertical).enumerate()
            {
                let intersects = closed_chords_intersect(chord_h, chord_v);
                let dominates = strict_dominance(point_h, point_v);
                if intersects != dominates {
                    return Err(EmbeddingError::PairMismatch {
                        horizontal: left,
                        vertical: right,
                        intersects,
                        dominates,
                    });
                }
                if point_h
                    .coordinates
                    .iter()
                    .zip(point_v.coordinates)
                    .any(|(first, second)| *first == second)
                {
                    return Err(EmbeddingError::CrossSideCoordinateEquality {
                        horizontal: left,
                        vertical: right,
                    });
                }
            }
        }
        Ok(())
    }

    /// Materializes every strict-dominance edge.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError`] if a graph endpoint violates declared dimensions.
    pub fn explicit_graph(&self) -> Result<BipartiteGraph, EmbeddingError> {
        let mut graph = BipartiteGraph::new(self.horizontal.len(), self.vertical.len());
        for (left, &horizontal) in self.horizontal.iter().enumerate() {
            for (right, &vertical) in self.vertical.iter().enumerate() {
                if strict_dominance(horizontal, vertical) {
                    graph.add_edge(left, right)?;
                }
            }
        }
        Ok(graph)
    }
}

#[must_use]
pub fn strict_dominance(left: DominancePoint, right: DominancePoint) -> bool {
    left.coordinates
        .into_iter()
        .zip(right.coordinates)
        .all(|(first, second)| first < second)
}

fn coordinate_ranks(values: impl Iterator<Item = Coord>) -> HashMap<Coord, usize> {
    values
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(rank, value)| (value, rank))
        .collect()
}

fn twice_rank(ranks: &HashMap<Coord, usize>, value: Coord) -> Result<i128, EmbeddingError> {
    let rank = *ranks
        .get(&value)
        .ok_or(EmbeddingError::MissingCoordinateRank { value })?;
    i128::try_from(rank)
        .ok()
        .and_then(|rank| rank.checked_mul(2))
        .ok_or(EmbeddingError::ArithmeticOverflow)
}

fn alpha(
    chord: HorizontalChord,
    x_ranks: &HashMap<Coord, usize>,
    y_ranks: &HashMap<Coord, usize>,
) -> Result<DominancePoint, EmbeddingError> {
    let left = twice_rank(x_ranks, chord.left())?;
    let right = twice_rank(x_ranks, chord.right())?;
    let y = twice_rank(y_ranks, chord.y())?;
    Ok(DominancePoint {
        coordinates: [
            left,
            right
                .checked_neg()
                .ok_or(EmbeddingError::ArithmeticOverflow)?,
            y,
            y.checked_neg().ok_or(EmbeddingError::ArithmeticOverflow)?,
        ],
    })
}

fn beta(
    chord: VerticalChord,
    x_ranks: &HashMap<Coord, usize>,
    y_ranks: &HashMap<Coord, usize>,
) -> Result<DominancePoint, EmbeddingError> {
    let x = twice_rank(x_ranks, chord.x())?;
    let top = twice_rank(y_ranks, chord.top())?;
    let bottom = twice_rank(y_ranks, chord.bottom())?;
    Ok(DominancePoint {
        coordinates: [
            x.checked_add(1).ok_or(EmbeddingError::ArithmeticOverflow)?,
            x.checked_neg()
                .and_then(|value| value.checked_add(1))
                .ok_or(EmbeddingError::ArithmeticOverflow)?,
            top.checked_add(1)
                .ok_or(EmbeddingError::ArithmeticOverflow)?,
            bottom
                .checked_neg()
                .and_then(|value| value.checked_add(1))
                .ok_or(EmbeddingError::ArithmeticOverflow)?,
        ],
    })
}

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("coordinate {value} is missing from its rank map")]
    MissingCoordinateRank { value: Coord },
    #[error("rank parity encoding overflowed i128")]
    ArithmeticOverflow,
    #[error("embedding and chord-family dimensions differ")]
    DimensionMismatch,
    #[error(
        "pair ({horizontal}, {vertical}) disagrees: intersection={intersects}, dominance={dominates}"
    )]
    PairMismatch {
        horizontal: usize,
        vertical: usize,
        intersects: bool,
        dominates: bool,
    },
    #[error("embedded cross-side pair ({horizontal}, {vertical}) shares a coordinate")]
    CrossSideCoordinateEquality { horizontal: usize, vertical: usize },
    #[error(transparent)]
    Graph(#[from] GraphError),
}

#[cfg(test)]
mod tests {
    use rect_core::{HorizontalChord, HorizontalChordId, VerticalChord, VerticalChordId};

    use super::DominanceEmbedding;

    #[test]
    fn exhaustively_preserves_closed_endpoint_intersection() {
        let mut horizontal = Vec::new();
        let mut vertical = Vec::new();
        for y in -2..=2 {
            for left in -2..2 {
                for right in (left + 1)..=2 {
                    horizontal.push(
                        HorizontalChord::new(HorizontalChordId(horizontal.len()), left, right, y)
                            .unwrap(),
                    );
                }
            }
        }
        for x in -2..=2 {
            for bottom in -2..2 {
                for top in (bottom + 1)..=2 {
                    vertical.push(
                        VerticalChord::new(VerticalChordId(vertical.len()), x, bottom, top)
                            .unwrap(),
                    );
                }
            }
        }
        let embedding = DominanceEmbedding::new(&horizontal, &vertical).unwrap();
        embedding
            .assert_pairwise_equivalence(&horizontal, &vertical)
            .unwrap();
    }
}
