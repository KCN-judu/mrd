//! Definition-level map-of-lines cut index.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Unbounded};

use rect_core::Point;
use serde::{Deserialize, Serialize};

use crate::polygon::{HorizontalCutSegment, PolygonDirection, VerticalCutSegment};

/// Deliberately simple exact index used to validate the experimental backend.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Index {
    horizontal_by_y: BTreeMap<i64, BTreeSet<(i64, i64)>>,
    vertical_by_x: BTreeMap<i64, BTreeSet<(i64, i64)>>,
}

impl Index {
    #[must_use]
    pub fn contains_horizontal_ray(&self, point: Point, east: bool) -> bool {
        self.horizontal_by_y.get(&point.y).is_some_and(|segments| {
            segments.iter().any(|&(left, right)| {
                if east {
                    left <= point.x && point.x < right
                } else {
                    left < point.x && point.x <= right
                }
            })
        })
    }

    #[must_use]
    pub fn contains_vertical_ray(&self, point: Point, north: bool) -> bool {
        self.vertical_by_x.get(&point.x).is_some_and(|segments| {
            segments.iter().any(|&(bottom, top)| {
                if north {
                    bottom <= point.y && point.y < top
                } else {
                    bottom < point.y && point.y <= top
                }
            })
        })
    }

    pub fn insert_horizontal_with_intersections(
        &mut self,
        segment: HorizontalCutSegment,
    ) -> (bool, Vec<Point>) {
        let intersections = self
            .vertical_by_x
            .range(segment.left..=segment.right)
            .filter_map(|(&x, intervals)| {
                intervals
                    .iter()
                    .any(|&(bottom, top)| bottom <= segment.y && segment.y <= top)
                    .then_some(Point::new(x, segment.y))
            })
            .collect::<Vec<_>>();
        let inserted = self
            .horizontal_by_y
            .entry(segment.y)
            .or_default()
            .insert((segment.left, segment.right));
        (inserted, intersections)
    }

    pub fn insert_vertical_with_intersections(
        &mut self,
        segment: VerticalCutSegment,
    ) -> (bool, Vec<Point>) {
        let intersections = self
            .horizontal_by_y
            .range(segment.bottom..=segment.top)
            .filter_map(|(&y, intervals)| {
                intervals
                    .iter()
                    .any(|&(left, right)| left <= segment.x && segment.x <= right)
                    .then_some(Point::new(segment.x, y))
            })
            .collect::<Vec<_>>();
        let inserted = self
            .vertical_by_x
            .entry(segment.x)
            .or_default()
            .insert((segment.bottom, segment.top));
        (inserted, intersections)
    }

    #[must_use]
    pub fn horizontal_segments(&self) -> Vec<HorizontalCutSegment> {
        self.horizontal_by_y
            .iter()
            .flat_map(|(&y, intervals)| {
                intervals
                    .iter()
                    .map(move |&(left, right)| HorizontalCutSegment { left, right, y })
            })
            .collect()
    }

    #[must_use]
    pub fn vertical_segments(&self) -> Vec<VerticalCutSegment> {
        self.vertical_by_x
            .iter()
            .flat_map(|(&x, intervals)| {
                intervals
                    .iter()
                    .map(move |&(bottom, top)| VerticalCutSegment { x, bottom, top })
            })
            .collect()
    }

    pub(crate) fn nearest_blocker(
        &self,
        point: Point,
        direction: PolygonDirection,
    ) -> Option<Point> {
        match direction {
            PolygonDirection::East => {
                let perpendicular = self
                    .vertical_by_x
                    .range((Excluded(point.x), Unbounded))
                    .find_map(|(&x, intervals)| {
                        intervals
                            .iter()
                            .any(|&(bottom, top)| bottom <= point.y && point.y <= top)
                            .then_some(x)
                    });
                let collinear = self.horizontal_by_y.get(&point.y).and_then(|segments| {
                    segments
                        .iter()
                        .filter_map(|&(left, _)| (left > point.x).then_some(left))
                        .min()
                });
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .min()
                    .map(|x| Point::new(x, point.y))
            }
            PolygonDirection::West => {
                let perpendicular = self
                    .vertical_by_x
                    .range((Unbounded, Excluded(point.x)))
                    .rev()
                    .find_map(|(&x, intervals)| {
                        intervals
                            .iter()
                            .any(|&(bottom, top)| bottom <= point.y && point.y <= top)
                            .then_some(x)
                    });
                let collinear = self.horizontal_by_y.get(&point.y).and_then(|segments| {
                    segments
                        .iter()
                        .filter_map(|&(_, right)| (right < point.x).then_some(right))
                        .max()
                });
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .max()
                    .map(|x| Point::new(x, point.y))
            }
            PolygonDirection::North => {
                let perpendicular = self
                    .horizontal_by_y
                    .range((Excluded(point.y), Unbounded))
                    .find_map(|(&y, intervals)| {
                        intervals
                            .iter()
                            .any(|&(left, right)| left <= point.x && point.x <= right)
                            .then_some(y)
                    });
                let collinear = self.vertical_by_x.get(&point.x).and_then(|segments| {
                    segments
                        .iter()
                        .filter_map(|&(bottom, _)| (bottom > point.y).then_some(bottom))
                        .min()
                });
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .min()
                    .map(|y| Point::new(point.x, y))
            }
            PolygonDirection::South => {
                let perpendicular = self
                    .horizontal_by_y
                    .range((Unbounded, Excluded(point.y)))
                    .rev()
                    .find_map(|(&y, intervals)| {
                        intervals
                            .iter()
                            .any(|&(left, right)| left <= point.x && point.x <= right)
                            .then_some(y)
                    });
                let collinear = self.vertical_by_x.get(&point.x).and_then(|segments| {
                    segments
                        .iter()
                        .filter_map(|&(_, top)| (top < point.y).then_some(top))
                        .max()
                });
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .max()
                    .map(|y| Point::new(point.x, y))
            }
        }
    }
}
