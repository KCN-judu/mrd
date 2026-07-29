//! Definition-level horizontal-range intersection scan.

use std::collections::{BTreeMap, BTreeSet};

use rect_core::Point;

use super::Backend;
use super::graph::{Metrics, Segment, initial_split_coordinates, record_intersection};

pub(super) fn split(segments: &[Segment]) -> (Vec<BTreeSet<i64>>, BTreeSet<Point>, Metrics) {
    let mut split_coordinates = initial_split_coordinates(segments);
    let mut junctions = BTreeSet::new();
    let mut vertical_by_x = BTreeMap::<i64, Vec<usize>>::new();
    let mut horizontal_ids = Vec::new();
    for (id, segment) in segments.iter().copied().enumerate() {
        if segment.horizontal() {
            horizontal_ids.push(id);
        } else {
            vertical_by_x.entry(segment.line()).or_default().push(id);
        }
    }
    let mut metrics = Metrics {
        builder_backend: Backend::Oracle.name().to_owned(),
        input_segment_count: segments.len(),
        horizontal_segment_count: horizontal_ids.len(),
        vertical_segment_count: segments.len() - horizontal_ids.len(),
        ..Metrics::default()
    };
    for horizontal_id in horizontal_ids {
        let horizontal = segments[horizontal_id];
        for (&x, vertical_ids) in vertical_by_x.range(horizontal.low()..=horizontal.high()) {
            for &vertical_id in vertical_ids {
                metrics.candidate_pair_tests += 1;
                let vertical = segments[vertical_id];
                if vertical.low() <= horizontal.line() && horizontal.line() <= vertical.high() {
                    record_intersection(
                        segments,
                        &mut split_coordinates,
                        horizontal_id,
                        vertical_id,
                        Point::new(x, horizontal.line()),
                        &mut junctions,
                        &mut metrics,
                    );
                }
            }
        }
    }
    metrics.materialized_split_coordinates = split_coordinates.iter().map(BTreeSet::len).sum();
    (split_coordinates, junctions, metrics)
}
