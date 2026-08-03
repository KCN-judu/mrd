use std::{
    collections::{BTreeSet, HashMap},
    mem::size_of,
};

use graph::{BipartiteGraph, GraphError};
use mrd_domain::{Coord, HorizontalChord, VerticalChord, closed_chords_intersect};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct DominancePoint {
    pub coordinates: [i128; 4],
}

/// Coordinate construction used to embed one chord family.
///
/// `RankedCoordinates` is the permanent general-coordinate Oracle.
/// `DirectGridParity` is valid for the finite integer grid pipeline and uses
/// the direct even/odd formula without coordinate ranking.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddingCoordinateBackend {
    RankedCoordinates,
    DirectGridParity,
}

/// Structural counters for one embedding construction.
///
/// The ranked fields describe only coordinate-ranking work. In particular, a
/// direct-grid construction reports zero for all three fields rather than an
/// unavailable estimate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingMetrics {
    pub rank_sort_count: usize,
    pub rank_map_entry_count: usize,
    pub rank_map_owned_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DominanceEmbedding {
    pub horizontal: Vec<DominancePoint>,
    pub vertical: Vec<DominancePoint>,
    pub backend: EmbeddingCoordinateBackend,
    pub metrics: EmbeddingMetrics,
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
        Self::new_with_backend(
            horizontal_chords,
            vertical_chords,
            EmbeddingCoordinateBackend::RankedCoordinates,
        )
    }

    /// Builds one exact embedding through the selected coordinate backend.
    ///
    /// `DirectGridParity` must only be selected by the finite integer grid
    /// pipeline. It deliberately does not construct rank sets, maps, or sorted
    /// coordinate vectors.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError`] when checked coordinate arithmetic overflows
    /// or a ranked coordinate is missing.
    pub fn new_with_backend(
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        backend: EmbeddingCoordinateBackend,
    ) -> Result<Self, EmbeddingError> {
        match backend {
            EmbeddingCoordinateBackend::RankedCoordinates => {
                Self::ranked(horizontal_chords, vertical_chords)
            }
            EmbeddingCoordinateBackend::DirectGridParity => {
                Self::direct_grid_parity(horizontal_chords, vertical_chords)
            }
        }
    }

    fn ranked(
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
            backend: EmbeddingCoordinateBackend::RankedCoordinates,
            metrics: EmbeddingMetrics {
                rank_sort_count: 2,
                rank_map_entry_count: x_ranks.len() + y_ranks.len(),
                rank_map_owned_bytes: rank_map_owned_bytes(&x_ranks)
                    + rank_map_owned_bytes(&y_ranks),
            },
        })
    }

    fn direct_grid_parity(
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
    ) -> Result<Self, EmbeddingError> {
        let horizontal = horizontal_chords
            .iter()
            .map(|chord| direct_alpha(*chord))
            .collect::<Result<Vec<_>, _>>()?;
        let vertical = vertical_chords
            .iter()
            .map(|chord| direct_beta(*chord))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            horizontal,
            vertical,
            backend: EmbeddingCoordinateBackend::DirectGridParity,
            metrics: EmbeddingMetrics::default(),
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

fn rank_map_owned_bytes(map: &HashMap<Coord, usize>) -> usize {
    map.capacity().saturating_mul(size_of::<(Coord, usize)>())
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

fn doubled(value: Coord) -> Result<i128, EmbeddingError> {
    i128::from(value)
        .checked_mul(2)
        .ok_or(EmbeddingError::ArithmeticOverflow)
}

fn direct_alpha(chord: HorizontalChord) -> Result<DominancePoint, EmbeddingError> {
    let left = doubled(chord.left())?;
    let right = doubled(chord.right())?;
    let y = doubled(chord.y())?;
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

fn direct_beta(chord: VerticalChord) -> Result<DominancePoint, EmbeddingError> {
    let x = doubled(chord.x())?;
    let top = doubled(chord.top())?;
    let bottom = doubled(chord.bottom())?;
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
    use mrd_domain::{HorizontalChord, HorizontalChordId, VerticalChord, VerticalChordId};

    use super::{DominanceEmbedding, EmbeddingCoordinateBackend};

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
        let ranked = DominanceEmbedding::new(&horizontal, &vertical).unwrap();
        ranked
            .assert_pairwise_equivalence(&horizontal, &vertical)
            .unwrap();
        let direct = DominanceEmbedding::new_with_backend(
            &horizontal,
            &vertical,
            EmbeddingCoordinateBackend::DirectGridParity,
        )
        .unwrap();
        direct
            .assert_pairwise_equivalence(&horizontal, &vertical)
            .unwrap();
        assert_eq!(
            direct.explicit_graph().unwrap(),
            ranked.explicit_graph().unwrap()
        );
        assert_eq!(ranked.metrics.rank_sort_count, 2);
        assert!(ranked.metrics.rank_map_entry_count > 0);
        assert!(ranked.metrics.rank_map_owned_bytes > 0);
        assert_eq!(direct.metrics.rank_sort_count, 0);
        assert_eq!(direct.metrics.rank_map_entry_count, 0);
        assert_eq!(direct.metrics.rank_map_owned_bytes, 0);
    }

    #[test]
    fn direct_grid_parity_uses_the_declared_even_odd_formula() {
        let horizontal = [HorizontalChord::new(HorizontalChordId(0), 1, 3, 2).unwrap()];
        let vertical = [VerticalChord::new(VerticalChordId(0), 2, 0, 4).unwrap()];
        let embedding = DominanceEmbedding::new_with_backend(
            &horizontal,
            &vertical,
            EmbeddingCoordinateBackend::DirectGridParity,
        )
        .unwrap();
        assert_eq!(embedding.horizontal[0].coordinates, [2, -6, 4, -4]);
        assert_eq!(embedding.vertical[0].coordinates, [5, -3, 9, 1]);
    }
}
