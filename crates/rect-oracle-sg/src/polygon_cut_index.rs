//! Dynamic exact cut indexes for the polygon completion frontier.
//!
//! `DynamicPolygonCutIndex` in `polygon.rs` is retained as the deliberately
//! simple line-map oracle.  The production index here uses the finite
//! completion-coordinate universe proved in
//! `docs/POLYGON_COMPLETION_COORDINATE_CLOSURE.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Included, Unbounded};

use rect_core::Point;
use serde::{Deserialize, Serialize};

use crate::polygon::{
    DynamicPolygonCutIndex, HorizontalCutSegment, PolygonDirection, PolygonSgError,
    VerticalCutSegment,
};

/// Selects the mutable cut index used by indexed polygon completion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonCutIndexBackend {
    /// Preserved map-of-lines implementation.  This is a correctness oracle.
    ReferenceLineMaps,
    /// Coordinate-compressed interval stabbing index with no line scans.
    #[default]
    DynamicStabbing,
}

impl PolygonCutIndexBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferenceLineMaps => "line-map-reference",
            Self::DynamicStabbing => "dynamic-stabbing",
        }
    }
}

/// Work performed by a mutable cut index.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CutIndexMetrics {
    pub insertions: usize,
    pub canonical_node_insertions: usize,
    pub stabbing_queries: usize,
    pub tree_node_visits: usize,
    pub ordered_set_queries: usize,
    pub reported_intersections: usize,
    pub coordinate_line_scans: usize,
    pub interval_scans: usize,
    pub owned_bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum CompletionCutIndex {
    Reference {
        index: DynamicPolygonCutIndex,
        metrics: CutIndexMetrics,
    },
    Dynamic(Box<DynamicStabbingCutIndex>),
}

impl CompletionCutIndex {
    pub(crate) fn new(
        backend: PolygonCutIndexBackend,
        coordinates: BTreeSet<i64>,
    ) -> Result<Self, PolygonSgError> {
        match backend {
            PolygonCutIndexBackend::ReferenceLineMaps => Ok(Self::Reference {
                index: DynamicPolygonCutIndex::default(),
                metrics: CutIndexMetrics::default(),
            }),
            PolygonCutIndexBackend::DynamicStabbing => Ok(Self::Dynamic(Box::new(
                DynamicStabbingCutIndex::new(coordinates)?,
            ))),
        }
    }

    pub(crate) fn contains_horizontal_ray(&mut self, point: Point, east: bool) -> bool {
        match self {
            Self::Reference { index, metrics } => {
                metrics.ordered_set_queries += 1;
                index.contains_horizontal_ray(point, east)
            }
            Self::Dynamic(index) => index.contains_horizontal_ray(point, east),
        }
    }

    pub(crate) fn contains_vertical_ray(&mut self, point: Point, north: bool) -> bool {
        match self {
            Self::Reference { index, metrics } => {
                metrics.ordered_set_queries += 1;
                index.contains_vertical_ray(point, north)
            }
            Self::Dynamic(index) => index.contains_vertical_ray(point, north),
        }
    }

    pub(crate) fn insert_horizontal_with_intersections(
        &mut self,
        segment: HorizontalCutSegment,
    ) -> Result<(bool, Vec<Point>), PolygonSgError> {
        match self {
            Self::Reference { index, metrics } => {
                metrics.insertions += 1;
                // The reference deliberately scans every opposite coordinate
                // line in range; retain that fact in its diagnostics.
                metrics.coordinate_line_scans += 1;
                Ok(index.insert_horizontal_with_intersections(segment))
            }
            Self::Dynamic(index) => index.insert_horizontal_with_intersections(segment),
        }
    }

    pub(crate) fn insert_vertical_with_intersections(
        &mut self,
        segment: VerticalCutSegment,
    ) -> Result<(bool, Vec<Point>), PolygonSgError> {
        match self {
            Self::Reference { index, metrics } => {
                metrics.insertions += 1;
                metrics.coordinate_line_scans += 1;
                Ok(index.insert_vertical_with_intersections(segment))
            }
            Self::Dynamic(index) => index.insert_vertical_with_intersections(segment),
        }
    }

    pub(crate) fn nearest_blocker(
        &mut self,
        point: Point,
        direction: PolygonDirection,
    ) -> Option<Point> {
        match self {
            Self::Reference { index, metrics } => {
                metrics.stabbing_queries += 1;
                metrics.coordinate_line_scans += 1;
                index.nearest_blocker(point, direction)
            }
            Self::Dynamic(index) => index.nearest_blocker(point, direction),
        }
    }

    #[must_use]
    pub(crate) fn horizontal_segments(&self) -> Vec<HorizontalCutSegment> {
        match self {
            Self::Reference { index, .. } => index.horizontal_segments(),
            Self::Dynamic(index) => index.horizontal_segments(),
        }
    }

    #[must_use]
    pub(crate) fn vertical_segments(&self) -> Vec<VerticalCutSegment> {
        match self {
            Self::Reference { index, .. } => index.vertical_segments(),
            Self::Dynamic(index) => index.vertical_segments(),
        }
    }

    #[must_use]
    pub(crate) fn metrics(&self) -> CutIndexMetrics {
        match self {
            Self::Reference { metrics, .. } => metrics.clone(),
            Self::Dynamic(index) => index.metrics(),
        }
    }
}

/// Per-coordinate canonical intervals.  Updates touch only neighboring
/// intervals, and point/ray lookups use predecessor/successor operations.
#[derive(Clone, Debug, Default)]
struct CollinearIntervalIndex {
    lines: BTreeMap<i64, BTreeSet<(i64, i64)>>,
}

impl CollinearIntervalIndex {
    fn insert(&mut self, line: i64, mut low: i64, mut high: i64) {
        let intervals = self.lines.entry(line).or_default();
        let predecessor = intervals
            .range((Unbounded, Included((low, i64::MAX))))
            .next_back()
            .copied();
        if let Some((start, end)) = predecessor
            && end >= low
        {
            low = low.min(start);
            high = high.max(end);
            intervals.remove(&(start, end));
        }
        loop {
            let successor = intervals
                .range((Included((low, i64::MIN)), Unbounded))
                .next()
                .copied();
            let Some((start, end)) = successor else {
                break;
            };
            if start > high {
                break;
            }
            high = high.max(end);
            intervals.remove(&(start, end));
        }
        intervals.insert((low, high));
    }

    fn contains_ray(&self, line: i64, point: i64, increasing: bool) -> bool {
        self.lines.get(&line).is_some_and(|intervals| {
            intervals
                .range((Unbounded, Included((point, i64::MAX))))
                .next_back()
                .is_some_and(|&(low, high)| {
                    if increasing {
                        low <= point && point < high
                    } else {
                        low < point && point <= high
                    }
                })
        })
    }

    fn nearest_endpoint(&self, line: i64, point: i64, increasing: bool) -> Option<i64> {
        let intervals = self.lines.get(&line)?;
        if increasing {
            intervals
                .range((Excluded((point, i64::MAX)), Unbounded))
                .next()
                .map(|&(low, _)| low)
        } else {
            let candidate = intervals
                .range((Unbounded, Excluded((point, i64::MIN))))
                .next_back()
                .copied()?;
            if candidate.1 < point {
                Some(candidate.1)
            } else {
                intervals
                    .range((Unbounded, Excluded((candidate.0, i64::MIN))))
                    .next_back()
                    .map(|&(_, high)| high)
            }
        }
    }

    fn horizontal_segments(&self) -> Vec<HorizontalCutSegment> {
        self.lines
            .iter()
            .flat_map(|(&y, intervals)| {
                intervals
                    .iter()
                    .map(move |&(left, right)| HorizontalCutSegment { left, right, y })
            })
            .collect()
    }

    fn vertical_segments(&self) -> Vec<VerticalCutSegment> {
        self.lines
            .iter()
            .flat_map(|(&x, intervals)| {
                intervals
                    .iter()
                    .map(move |&(bottom, top)| VerticalCutSegment { x, bottom, top })
            })
            .collect()
    }

    fn owned_bytes_estimate(&self) -> usize {
        self.lines
            .values()
            .map(|intervals| intervals.len() * std::mem::size_of::<(i64, i64)>())
            .sum()
    }
}

/// Insert-only segment tree for exact coordinate point stabbing.
#[derive(Clone, Debug)]
struct DynamicAxisStabbingIndex {
    coordinates: Vec<i64>,
    nodes: Vec<BTreeSet<(i64, usize)>>,
}

impl DynamicAxisStabbingIndex {
    fn new(coordinates: Vec<i64>) -> Result<Self, PolygonSgError> {
        if coordinates.is_empty() {
            return Err(PolygonSgError::CoordinateOverflow);
        }
        Ok(Self {
            nodes: vec![BTreeSet::new(); coordinates.len().saturating_mul(4)],
            coordinates,
        })
    }

    fn coordinate_index(&self, coordinate: i64) -> Result<usize, PolygonSgError> {
        self.coordinates
            .binary_search(&coordinate)
            .map_err(|_| PolygonSgError::CompletionCoordinateOutsideUniverse { coordinate })
    }

    fn insert(
        &mut self,
        low: i64,
        high: i64,
        key: i64,
        id: usize,
        metrics: &mut CutIndexMetrics,
    ) -> Result<(), PolygonSgError> {
        let low = self.coordinate_index(low)?;
        let high = self.coordinate_index(high)?;
        self.insert_range(
            1,
            0,
            self.coordinates.len() - 1,
            low,
            high,
            key,
            id,
            metrics,
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_range(
        &mut self,
        node: usize,
        start: usize,
        end: usize,
        low: usize,
        high: usize,
        key: i64,
        id: usize,
        metrics: &mut CutIndexMetrics,
    ) {
        if low <= start && end <= high {
            self.nodes[node].insert((key, id));
            metrics.canonical_node_insertions += 1;
            return;
        }
        let middle = start + (end - start) / 2;
        if low <= middle {
            self.insert_range(node * 2, start, middle, low, high, key, id, metrics);
        }
        if high > middle {
            self.insert_range(node * 2 + 1, middle + 1, end, low, high, key, id, metrics);
        }
    }

    fn path_nodes(&self, coordinate: i64, metrics: &mut CutIndexMetrics) -> Option<Vec<usize>> {
        let mut node = 1;
        let mut start = 0;
        let mut end = self.coordinates.len().checked_sub(1)?;
        let target = self.coordinates.binary_search(&coordinate).ok()?;
        let mut path = Vec::new();
        loop {
            path.push(node);
            metrics.tree_node_visits += 1;
            if start == end {
                return Some(path);
            }
            let middle = start + (end - start) / 2;
            if target <= middle {
                node *= 2;
                end = middle;
            } else {
                node = node * 2 + 1;
                start = middle + 1;
            }
        }
    }

    fn nearest(
        &self,
        coordinate: i64,
        key: i64,
        increasing: bool,
        metrics: &mut CutIndexMetrics,
    ) -> Option<i64> {
        metrics.stabbing_queries += 1;
        self.path_nodes(coordinate, metrics)?
            .into_iter()
            .filter_map(|node| {
                metrics.ordered_set_queries += 1;
                if increasing {
                    self.nodes[node]
                        .range((Excluded((key, usize::MAX)), Unbounded))
                        .next()
                        .map(|&(candidate, _)| candidate)
                } else {
                    self.nodes[node]
                        .range((Unbounded, Excluded((key, 0))))
                        .next_back()
                        .map(|&(candidate, _)| candidate)
                }
            })
            .reduce(if increasing { i64::min } else { i64::max })
    }

    fn report(
        &self,
        coordinate: i64,
        low: i64,
        high: i64,
        metrics: &mut CutIndexMetrics,
    ) -> Vec<usize> {
        let Some(path) = self.path_nodes(coordinate, metrics) else {
            return Vec::new();
        };
        metrics.stabbing_queries += 1;
        let mut ids = BTreeSet::new();
        for node in path {
            metrics.ordered_set_queries += 1;
            ids.extend(
                self.nodes[node]
                    .range((Included((low, 0)), Included((high, usize::MAX))))
                    .map(|&(_, id)| id),
            );
        }
        ids.into_iter().collect()
    }

    fn owned_bytes_estimate(&self) -> usize {
        self.coordinates.len() * std::mem::size_of::<i64>()
            + self
                .nodes
                .iter()
                .map(|entries| entries.len() * std::mem::size_of::<(i64, usize)>())
                .sum::<usize>()
    }
}

/// Exact dynamic orthogonal index.  Segments are inserted only; the
/// completion policy never deletes cuts.
#[derive(Clone, Debug)]
pub struct DynamicStabbingCutIndex {
    horizontal: CollinearIntervalIndex,
    vertical: CollinearIntervalIndex,
    vertical_stabbing: DynamicAxisStabbingIndex,
    horizontal_stabbing: DynamicAxisStabbingIndex,
    vertical_segments: Vec<VerticalCutSegment>,
    horizontal_segments: Vec<HorizontalCutSegment>,
    seen_horizontal: BTreeSet<HorizontalCutSegment>,
    seen_vertical: BTreeSet<VerticalCutSegment>,
    universe: BTreeSet<i64>,
    metrics: CutIndexMetrics,
}

impl DynamicStabbingCutIndex {
    /// Builds an index over the statically closed completion-coordinate set.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError::CoordinateOverflow`] when the universe is
    /// empty or cannot be represented by the index.
    pub fn new(universe: BTreeSet<i64>) -> Result<Self, PolygonSgError> {
        let coordinates = universe.iter().copied().collect::<Vec<_>>();
        let vertical_stabbing = DynamicAxisStabbingIndex::new(coordinates.clone())?;
        let horizontal_stabbing = DynamicAxisStabbingIndex::new(coordinates.clone())?;
        Ok(Self {
            horizontal: CollinearIntervalIndex::default(),
            vertical: CollinearIntervalIndex::default(),
            vertical_stabbing,
            horizontal_stabbing,
            vertical_segments: Vec::new(),
            horizontal_segments: Vec::new(),
            seen_horizontal: BTreeSet::new(),
            seen_vertical: BTreeSet::new(),
            universe,
            metrics: CutIndexMetrics::default(),
        })
    }

    fn ensure_coordinate(&self, coordinate: i64) -> Result<(), PolygonSgError> {
        if self.universe.contains(&coordinate) {
            Ok(())
        } else {
            Err(PolygonSgError::CompletionCoordinateOutsideUniverse { coordinate })
        }
    }

    fn ensure_point(&self, point: Point) -> Result<(), PolygonSgError> {
        self.ensure_coordinate(point.x)?;
        self.ensure_coordinate(point.y)
    }

    fn contains_horizontal_ray(&mut self, point: Point, east: bool) -> bool {
        self.metrics.ordered_set_queries += 1;
        self.horizontal.contains_ray(point.y, point.x, east)
    }

    fn contains_vertical_ray(&mut self, point: Point, north: bool) -> bool {
        self.metrics.ordered_set_queries += 1;
        self.vertical.contains_ray(point.x, point.y, north)
    }

    fn insert_horizontal_with_intersections(
        &mut self,
        segment: HorizontalCutSegment,
    ) -> Result<(bool, Vec<Point>), PolygonSgError> {
        self.ensure_coordinate(segment.left)?;
        self.ensure_coordinate(segment.right)?;
        self.ensure_coordinate(segment.y)?;
        if !self.seen_horizontal.insert(segment) {
            return Ok((false, Vec::new()));
        }
        self.metrics.insertions += 1;
        let intersections = self
            .vertical_stabbing
            .report(segment.y, segment.left, segment.right, &mut self.metrics)
            .into_iter()
            .filter_map(|id| self.vertical_segments.get(id).copied())
            .filter(|vertical| vertical.bottom <= segment.y && segment.y <= vertical.top)
            .map(|vertical| Point::new(vertical.x, segment.y))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        self.metrics.reported_intersections += intersections.len();
        self.horizontal
            .insert(segment.y, segment.left, segment.right);
        let id = self.horizontal_segments.len();
        self.horizontal_segments.push(segment);
        self.horizontal_stabbing.insert(
            segment.left,
            segment.right,
            segment.y,
            id,
            &mut self.metrics,
        )?;
        self.refresh_owned_bytes();
        Ok((true, intersections))
    }

    fn insert_vertical_with_intersections(
        &mut self,
        segment: VerticalCutSegment,
    ) -> Result<(bool, Vec<Point>), PolygonSgError> {
        self.ensure_coordinate(segment.x)?;
        self.ensure_coordinate(segment.bottom)?;
        self.ensure_coordinate(segment.top)?;
        if !self.seen_vertical.insert(segment) {
            return Ok((false, Vec::new()));
        }
        self.metrics.insertions += 1;
        let intersections = self
            .horizontal_stabbing
            .report(segment.x, segment.bottom, segment.top, &mut self.metrics)
            .into_iter()
            .filter_map(|id| self.horizontal_segments.get(id).copied())
            .filter(|horizontal| horizontal.left <= segment.x && segment.x <= horizontal.right)
            .map(|horizontal| Point::new(segment.x, horizontal.y))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        self.metrics.reported_intersections += intersections.len();
        self.vertical.insert(segment.x, segment.bottom, segment.top);
        let id = self.vertical_segments.len();
        self.vertical_segments.push(segment);
        self.vertical_stabbing.insert(
            segment.bottom,
            segment.top,
            segment.x,
            id,
            &mut self.metrics,
        )?;
        self.refresh_owned_bytes();
        Ok((true, intersections))
    }

    fn nearest_blocker(&mut self, point: Point, direction: PolygonDirection) -> Option<Point> {
        let blocker = match direction {
            PolygonDirection::East => {
                let perpendicular =
                    self.vertical_stabbing
                        .nearest(point.y, point.x, true, &mut self.metrics);
                self.horizontal
                    .nearest_endpoint(point.y, point.x, true)
                    .into_iter()
                    .chain(perpendicular)
                    .min()
                    .map(|x| Point::new(x, point.y))
            }
            PolygonDirection::West => {
                let perpendicular =
                    self.vertical_stabbing
                        .nearest(point.y, point.x, false, &mut self.metrics);
                self.horizontal
                    .nearest_endpoint(point.y, point.x, false)
                    .into_iter()
                    .chain(perpendicular)
                    .max()
                    .map(|x| Point::new(x, point.y))
            }
            PolygonDirection::North => {
                let perpendicular =
                    self.horizontal_stabbing
                        .nearest(point.x, point.y, true, &mut self.metrics);
                self.vertical
                    .nearest_endpoint(point.x, point.y, true)
                    .into_iter()
                    .chain(perpendicular)
                    .min()
                    .map(|y| Point::new(point.x, y))
            }
            PolygonDirection::South => {
                let perpendicular =
                    self.horizontal_stabbing
                        .nearest(point.x, point.y, false, &mut self.metrics);
                self.vertical
                    .nearest_endpoint(point.x, point.y, false)
                    .into_iter()
                    .chain(perpendicular)
                    .max()
                    .map(|y| Point::new(point.x, y))
            }
        };
        if let Some(stop) = blocker {
            debug_assert!(self.ensure_point(stop).is_ok());
        }
        blocker
    }

    fn horizontal_segments(&self) -> Vec<HorizontalCutSegment> {
        self.horizontal.horizontal_segments()
    }

    fn vertical_segments(&self) -> Vec<VerticalCutSegment> {
        self.vertical.vertical_segments()
    }

    fn refresh_owned_bytes(&mut self) {
        self.metrics.owned_bytes = self.universe.len() * std::mem::size_of::<i64>()
            + self.horizontal.owned_bytes_estimate()
            + self.vertical.owned_bytes_estimate()
            + self.vertical_stabbing.owned_bytes_estimate()
            + self.horizontal_stabbing.owned_bytes_estimate()
            + (self.vertical_segments.len() * std::mem::size_of::<VerticalCutSegment>())
            + (self.horizontal_segments.len() * std::mem::size_of::<HorizontalCutSegment>());
    }

    fn metrics(&self) -> CutIndexMetrics {
        self.metrics.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rect_core::Point;

    use crate::polygon::{HorizontalCutSegment, VerticalCutSegment};

    use super::DynamicStabbingCutIndex;

    #[test]
    fn dynamic_stabbing_reports_intersection_and_nearest_blocker() {
        let mut index = DynamicStabbingCutIndex::new(BTreeSet::from([1, 2, 5, 9])).unwrap();
        let horizontal = HorizontalCutSegment::new(1, 9, 5).unwrap();
        let vertical = VerticalCutSegment::new(5, 1, 9).unwrap();
        assert!(
            index
                .insert_horizontal_with_intersections(horizontal)
                .unwrap()
                .0
        );
        assert_eq!(
            index
                .insert_vertical_with_intersections(vertical)
                .unwrap()
                .1,
            vec![Point::new(5, 5)]
        );
        assert!(index.contains_horizontal_ray(Point::new(2, 5), true));
        assert!(index.contains_vertical_ray(Point::new(5, 2), true));
        assert_eq!(
            index.nearest_blocker(Point::new(2, 5), crate::polygon::PolygonDirection::East),
            Some(Point::new(5, 5))
        );
        let metrics = index.metrics();
        assert_eq!(metrics.coordinate_line_scans, 0);
        assert_eq!(metrics.interval_scans, 0);
    }

    #[test]
    fn dynamic_stabbing_rejects_coordinates_outside_the_closed_universe() {
        let mut index = DynamicStabbingCutIndex::new(BTreeSet::from([0, 1, 2])).unwrap();
        let error = index
            .insert_horizontal_with_intersections(HorizontalCutSegment::new(0, 3, 1).unwrap())
            .unwrap_err();
        assert!(matches!(
            error,
            crate::polygon::PolygonSgError::CompletionCoordinateOutsideUniverse { coordinate: 3 }
        ));
    }
}
