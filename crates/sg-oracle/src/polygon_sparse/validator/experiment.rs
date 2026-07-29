//! Event-driven vertical slab segment-tree validator.

use std::collections::{BTreeMap, BTreeSet};

use mrd_domain::{CoordinateRect, DoubledPoint, MemoryEstimate, RectilinearPolygon};

use crate::polygon::PolygonValidationError;

use super::{Backend, Metrics};

/// Validates with an event-driven parity and rectangle-coverage tree.
///
/// # Errors
///
/// Returns the first exact geometry, coverage, or area error.
#[allow(clippy::too_many_lines)]
pub fn validate(
    polygon: &RectilinearPolygon,
    rectangles: &[CoordinateRect],
) -> Result<Metrics, PolygonValidationError> {
    let polygon_area_twice = polygon
        .twice_signed_area()
        .map_err(PolygonValidationError::Polygon)?;
    let mut rectangle_area = 0_i128;
    let mut y_coordinates = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
        .collect::<BTreeSet<_>>();
    let mut events = BTreeMap::<i64, Vec<SlabEvent>>::new();
    for boundary_loop in polygon.loops() {
        for (first, second) in boundary_loop.edges() {
            if first.y != second.y {
                continue;
            }
            let left = first.x.min(second.x);
            let right = first.x.max(second.x);
            events
                .entry(left)
                .or_default()
                .push(SlabEvent::PolygonToggle(first.y));
            events
                .entry(right)
                .or_default()
                .push(SlabEvent::PolygonToggle(first.y));
        }
    }
    for (index, rectangle) in rectangles.iter().copied().enumerate() {
        if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
            return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
        }
        rectangle_area = rectangle_area
            .checked_add(rectangle.area())
            .ok_or(PolygonValidationError::AreaOverflow)?;
        y_coordinates.extend([rectangle.y0, rectangle.y1]);
        events
            .entry(rectangle.x0)
            .or_default()
            .push(SlabEvent::RectangleStart {
                bottom: rectangle.y0,
                top: rectangle.y1,
            });
        events
            .entry(rectangle.x1)
            .or_default()
            .push(SlabEvent::RectangleEnd {
                bottom: rectangle.y0,
                top: rectangle.y1,
            });
    }
    let rectangle_area_twice = rectangle_area
        .checked_mul(2)
        .ok_or(PolygonValidationError::AreaOverflow)?;
    if rectangle_area_twice != polygon_area_twice {
        return Err(PolygonValidationError::AreaMismatch {
            polygon_area_twice,
            rectangle_area_twice,
        });
    }

    let y_coordinates = y_coordinates.into_iter().collect::<Vec<_>>();
    let mut tree = ValidationSegmentTree::new(y_coordinates.len().saturating_sub(1));
    let x_coordinates = events.keys().copied().collect::<Vec<_>>();
    let mut metrics = Metrics {
        validator_backend: Backend::Experiment.name().to_owned(),
        x_event_count: events.values().map(Vec::len).sum(),
        y_coordinate_count: y_coordinates.len(),
        owned_bytes: y_coordinates.capacity() * std::mem::size_of::<i64>()
            + tree.owned_bytes_estimate(),
        ..Metrics::default()
    };
    metrics.memory_estimate = MemoryEstimate {
        retained_payload_bytes: y_coordinates.len() * std::mem::size_of::<i64>()
            + tree.nodes.len() * std::mem::size_of::<ValidationNode>(),
        retained_collection_capacity_bytes: (y_coordinates.capacity() - y_coordinates.len())
            * std::mem::size_of::<i64>()
            + (tree.nodes.capacity() - tree.nodes.len()) * std::mem::size_of::<ValidationNode>(),
        retained_container_estimate: events.len() * std::mem::size_of::<Vec<SlabEvent>>(),
        peak_temporary_payload_bytes: x_coordinates.capacity() * std::mem::size_of::<i64>(),
        unmeasured_allocator_overhead: true,
    };
    metrics.owned_bytes = metrics.memory_estimate.retained_total_estimate();
    for pair in x_coordinates.windows(2) {
        let x = pair[0];
        let Some(changes) = events.get_mut(&x) else {
            continue;
        };
        changes.sort_unstable();
        for event in changes.iter().copied() {
            match event {
                SlabEvent::PolygonToggle(y) => {
                    let start = y_coordinates
                        .binary_search(&y)
                        .map_err(|_| PolygonValidationError::AreaOverflow)?;
                    if start + 1 < y_coordinates.len() {
                        tree.toggle(start, y_coordinates.len() - 2, &mut metrics);
                        metrics.parity_toggle_count += 1;
                    }
                }
                SlabEvent::RectangleEnd { bottom, top } => {
                    update_rectangle_coverage(
                        &mut tree,
                        &y_coordinates,
                        bottom,
                        top,
                        -1,
                        &mut metrics,
                    )?;
                }
                SlabEvent::RectangleStart { bottom, top } => {
                    update_rectangle_coverage(
                        &mut tree,
                        &y_coordinates,
                        bottom,
                        top,
                        1,
                        &mut metrics,
                    )?;
                }
            }
        }
        if pair[0] == pair[1] || tree.leaf_count == 0 {
            continue;
        }
        metrics.slab_count += 1;
        metrics.root_checks += 1;
        let doubled_x = i128::from(pair[0]) + i128::from(pair[1]);
        if let Some(leaf) = tree.first_leaf(ValidationViolation::Overlap, &mut metrics) {
            let point = slab_witness(doubled_x, &y_coordinates, leaf);
            let covering = covering_rectangles(rectangles, point);
            return Err(PolygonValidationError::Overlap {
                first: covering[0],
                second: covering[1],
                point,
            });
        }
        if let Some(leaf) = tree.first_leaf(ValidationViolation::Uncovered, &mut metrics) {
            return Err(PolygonValidationError::UncoveredInterior {
                point: slab_witness(doubled_x, &y_coordinates, leaf),
            });
        }
        if let Some(leaf) = tree.first_leaf(ValidationViolation::Outside, &mut metrics) {
            let point = slab_witness(doubled_x, &y_coordinates, leaf);
            return Err(PolygonValidationError::OutsidePolygon {
                rectangle: covering_rectangles(rectangles, point)[0],
                point,
            });
        }
    }
    Ok(metrics)
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SlabEvent {
    PolygonToggle(i64),
    RectangleEnd { bottom: i64, top: i64 },
    RectangleStart { bottom: i64, top: i64 },
}

#[derive(Clone, Copy, Debug, Default)]
struct ValidationNode {
    present: [bool; 2],
    minimum: [i32; 2],
    maximum: [i32; 2],
    lazy_add: i32,
    lazy_toggle: bool,
}

#[derive(Clone, Copy)]
enum ValidationViolation {
    Overlap,
    Uncovered,
    Outside,
}

struct ValidationSegmentTree {
    nodes: Vec<ValidationNode>,
    leaf_count: usize,
}

impl ValidationSegmentTree {
    fn new(leaf_count: usize) -> Self {
        let mut tree = Self {
            nodes: vec![ValidationNode::default(); leaf_count.saturating_mul(4).max(1)],
            leaf_count,
        };
        if leaf_count > 0 {
            tree.build(1, 0, leaf_count - 1);
        }
        tree
    }

    fn build(&mut self, node: usize, start: usize, end: usize) {
        if start == end {
            self.nodes[node].present[0] = true;
            return;
        }
        let middle = start + (end - start) / 2;
        self.build(node * 2, start, middle);
        self.build(node * 2 + 1, middle + 1, end);
        self.pull(node);
    }

    fn owned_bytes_estimate(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<ValidationNode>()
    }

    fn add(&mut self, low: usize, high: usize, delta: i32, metrics: &mut Metrics) {
        self.update_add(1, 0, self.leaf_count - 1, low, high, delta, metrics);
    }

    fn toggle(&mut self, low: usize, high: usize, metrics: &mut Metrics) {
        self.update_toggle(1, 0, self.leaf_count - 1, low, high, metrics);
    }

    fn apply_add(&mut self, node: usize, delta: i32) {
        for parity in 0..2 {
            if self.nodes[node].present[parity] {
                self.nodes[node].minimum[parity] += delta;
                self.nodes[node].maximum[parity] += delta;
            }
        }
        self.nodes[node].lazy_add += delta;
    }

    fn apply_toggle(&mut self, node: usize) {
        self.nodes[node].present.swap(0, 1);
        self.nodes[node].minimum.swap(0, 1);
        self.nodes[node].maximum.swap(0, 1);
        self.nodes[node].lazy_toggle = !self.nodes[node].lazy_toggle;
    }

    fn push(&mut self, node: usize) {
        if self.nodes[node].lazy_toggle {
            self.apply_toggle(node * 2);
            self.apply_toggle(node * 2 + 1);
            self.nodes[node].lazy_toggle = false;
        }
        let delta = self.nodes[node].lazy_add;
        if delta != 0 {
            self.apply_add(node * 2, delta);
            self.apply_add(node * 2 + 1, delta);
            self.nodes[node].lazy_add = 0;
        }
    }

    fn pull(&mut self, node: usize) {
        for parity in 0..2 {
            let left = self.nodes[node * 2];
            let right = self.nodes[node * 2 + 1];
            self.nodes[node].present[parity] = left.present[parity] || right.present[parity];
            self.nodes[node].minimum[parity] = if left.present[parity] && right.present[parity] {
                left.minimum[parity].min(right.minimum[parity])
            } else if left.present[parity] {
                left.minimum[parity]
            } else {
                right.minimum[parity]
            };
            self.nodes[node].maximum[parity] = if left.present[parity] && right.present[parity] {
                left.maximum[parity].max(right.maximum[parity])
            } else if left.present[parity] {
                left.maximum[parity]
            } else {
                right.maximum[parity]
            };
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn update_add(
        &mut self,
        node: usize,
        start: usize,
        end: usize,
        low: usize,
        high: usize,
        delta: i32,
        metrics: &mut Metrics,
    ) {
        metrics.segment_tree_node_visits += 1;
        if low <= start && end <= high {
            self.apply_add(node, delta);
            return;
        }
        self.push(node);
        let middle = start + (end - start) / 2;
        if low <= middle {
            self.update_add(node * 2, start, middle, low, high, delta, metrics);
        }
        if high > middle {
            self.update_add(node * 2 + 1, middle + 1, end, low, high, delta, metrics);
        }
        self.pull(node);
    }

    fn update_toggle(
        &mut self,
        node: usize,
        start: usize,
        end: usize,
        low: usize,
        high: usize,
        metrics: &mut Metrics,
    ) {
        metrics.segment_tree_node_visits += 1;
        if low <= start && end <= high {
            self.apply_toggle(node);
            return;
        }
        self.push(node);
        let middle = start + (end - start) / 2;
        if low <= middle {
            self.update_toggle(node * 2, start, middle, low, high, metrics);
        }
        if high > middle {
            self.update_toggle(node * 2 + 1, middle + 1, end, low, high, metrics);
        }
        self.pull(node);
    }

    fn violates(node: ValidationNode, violation: ValidationViolation) -> bool {
        match violation {
            ValidationViolation::Overlap => {
                (node.present[0] && node.maximum[0] > 1) || (node.present[1] && node.maximum[1] > 1)
            }
            ValidationViolation::Uncovered => node.present[1] && node.minimum[1] == 0,
            ValidationViolation::Outside => node.present[0] && node.maximum[0] > 0,
        }
    }

    fn first_leaf(
        &mut self,
        violation: ValidationViolation,
        metrics: &mut Metrics,
    ) -> Option<usize> {
        Self::violates(self.nodes[1], violation)
            .then(|| self.find_first(1, 0, self.leaf_count - 1, violation, metrics))
    }

    fn find_first(
        &mut self,
        node: usize,
        start: usize,
        end: usize,
        violation: ValidationViolation,
        metrics: &mut Metrics,
    ) -> usize {
        metrics.segment_tree_node_visits += 1;
        if start == end {
            return start;
        }
        self.push(node);
        let middle = start + (end - start) / 2;
        if Self::violates(self.nodes[node * 2], violation) {
            self.find_first(node * 2, start, middle, violation, metrics)
        } else {
            self.find_first(node * 2 + 1, middle + 1, end, violation, metrics)
        }
    }
}

fn update_rectangle_coverage(
    tree: &mut ValidationSegmentTree,
    y_coordinates: &[i64],
    bottom: i64,
    top: i64,
    delta: i32,
    metrics: &mut Metrics,
) -> Result<(), PolygonValidationError> {
    let low = y_coordinates
        .binary_search(&bottom)
        .map_err(|_| PolygonValidationError::AreaOverflow)?;
    let high = y_coordinates
        .binary_search(&top)
        .map_err(|_| PolygonValidationError::AreaOverflow)?;
    if low < high {
        tree.add(low, high - 1, delta, metrics);
        metrics.range_add_count += 1;
    }
    Ok(())
}

fn slab_witness(doubled_x: i128, y_coordinates: &[i64], leaf: usize) -> DoubledPoint {
    DoubledPoint::new(
        doubled_x,
        i128::from(y_coordinates[leaf]) + i128::from(y_coordinates[leaf + 1]),
    )
}

fn covering_rectangles(rectangles: &[CoordinateRect], point: DoubledPoint) -> Vec<usize> {
    rectangles
        .iter()
        .enumerate()
        .filter_map(|(index, rectangle)| {
            (2 * i128::from(rectangle.x0) < point.x
                && point.x < 2 * i128::from(rectangle.x1)
                && 2 * i128::from(rectangle.y0) < point.y
                && point.y < 2 * i128::from(rectangle.y1))
            .then_some(index)
        })
        .collect()
}
