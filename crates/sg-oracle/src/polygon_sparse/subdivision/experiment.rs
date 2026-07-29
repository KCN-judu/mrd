//! Output-sensitive closed-endpoint orthogonal sweep.

use std::collections::BTreeSet;

use mrd_domain::Point;

use super::Backend;
use super::graph::{Metrics, Segment, initial_split_coordinates, record_intersection};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EventKind {
    HorizontalStart,
    VerticalQuery,
    HorizontalEnd,
}

pub(super) fn split(segments: &[Segment]) -> (Vec<BTreeSet<i64>>, BTreeSet<Point>, Metrics) {
    let mut split_coordinates = initial_split_coordinates(segments);
    let mut junctions = BTreeSet::new();
    let mut events = Vec::with_capacity(segments.len().saturating_mul(2));
    let mut horizontal_count = 0;
    for (id, segment) in segments.iter().copied().enumerate() {
        if segment.horizontal() {
            horizontal_count += 1;
            events.push((segment.low(), EventKind::HorizontalStart, id));
            events.push((segment.high(), EventKind::HorizontalEnd, id));
        } else {
            events.push((segment.line(), EventKind::VerticalQuery, id));
        }
    }
    events.sort_unstable();
    let mut active = BTreeSet::<(i64, usize)>::new();
    let mut metrics = Metrics {
        builder_backend: Backend::Experiment.name().to_owned(),
        input_segment_count: segments.len(),
        horizontal_segment_count: horizontal_count,
        vertical_segment_count: segments.len() - horizontal_count,
        sweep_event_count: events.len(),
        ..Metrics::default()
    };
    for (x, kind, id) in events {
        let segment = segments[id];
        match kind {
            EventKind::HorizontalStart => {
                active.insert((segment.line(), id));
                metrics.active_set_insertions += 1;
            }
            EventKind::VerticalQuery => {
                metrics.range_queries += 1;
                let intersections = active
                    .range((segment.low(), 0)..=(segment.high(), usize::MAX))
                    .copied()
                    .collect::<Vec<_>>();
                for (y, horizontal_id) in intersections {
                    record_intersection(
                        segments,
                        &mut split_coordinates,
                        horizontal_id,
                        id,
                        Point::new(x, y),
                        &mut junctions,
                        &mut metrics,
                    );
                }
            }
            EventKind::HorizontalEnd => {
                active.remove(&(segment.line(), id));
                metrics.active_set_removals += 1;
            }
        }
    }
    metrics.materialized_split_coordinates = split_coordinates.iter().map(BTreeSet::len).sum();
    (split_coordinates, junctions, metrics)
}
