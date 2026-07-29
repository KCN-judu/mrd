//! Source-mapped event-sweep chord enumeration.

#[allow(clippy::wildcard_imports)]
use super::super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Sweep;

#[derive(Default)]
struct SweepEventBucket {
    insertions: Vec<usize>,
    removals: Vec<usize>,
    queries: Vec<(Point, BoundaryVertexId)>,
}

impl Sweep {
    /// Enumerates effective chords with the source-mapped ordinary-polygon
    /// event sweep.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid prepared boundary metadata or
    /// chord-coordinate construction.
    pub fn enumerate(&self, polygon: &RectilinearPolygon) -> Result<Families, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            mrd_domain::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            mrd_domain::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        Ok(self.enumerate_prepared(&prepared)?.families)
    }

    /// Runs the axis-generic source-mapped event sweep on shared metadata.
    ///
    /// The accepted ordinary-loop model makes Definition 7's formal-boundary
    /// merge cases inapplicable; see `docs/SOLTAN_SWEEP_IMPLEMENTATION.md`.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid boundary identities or chord
    /// construction failure.
    pub fn enumerate_prepared(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<PolygonChordEnumerationResult, PolygonSgError> {
        let mut metrics = PolygonChordEnumerationMetrics::default();
        let horizontal_started = Instant::now();
        let (horizontal, mut certificate) =
            enumerate_sweep_axis(prepared, SweepAxis::Horizontal, &mut metrics)?;
        metrics.sweep_horizontal_microseconds = horizontal_started.elapsed().as_micros();
        let vertical_started = Instant::now();
        let (vertical, vertical_certificate) =
            enumerate_sweep_axis(prepared, SweepAxis::Vertical, &mut metrics)?;
        metrics.sweep_vertical_microseconds = vertical_started.elapsed().as_micros();
        certificate
            .output_records
            .extend(vertical_certificate.output_records);
        certificate
            .event_summaries
            .extend(vertical_certificate.event_summaries);
        certificate.event_trace_truncated |= vertical_certificate.event_trace_truncated;
        certificate.output_records.sort_unstable_by_key(|record| {
            (
                record.axis.name(),
                record.source_point,
                record.target_point,
                record.source,
                record.target,
            )
        });
        certificate
            .event_summaries
            .sort_unstable_by_key(|summary| (summary.axis.name(), summary.coordinate));

        let families = Families {
            horizontal: horizontal
                .into_iter()
                .enumerate()
                .map(|(index, (y, left, right))| {
                    HorizontalChord::new(HorizontalChordId(index), left, right, y)
                })
                .collect::<Result<Vec<_>, _>>()?,
            vertical: vertical
                .into_iter()
                .enumerate()
                .map(|(index, (x, bottom, top))| {
                    VerticalChord::new(VerticalChordId(index), x, bottom, top)
                })
                .collect::<Result<Vec<_>, _>>()?,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(0),
        };
        Ok(PolygonChordEnumerationResult {
            families,
            metrics,
            sweep_certificate: Some(certificate),
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "sg-sweep"
    }
}

#[allow(clippy::too_many_lines)]
fn enumerate_sweep_axis(
    prepared: &PreparedPolygonContext,
    axis: SweepAxis,
    metrics: &mut PolygonChordEnumerationMetrics,
) -> Result<(SweepChordKeys, SweepCertificate), PolygonSgError> {
    let boundary = prepared.boundary();
    let boundary_index = prepared.boundary_index();
    let edge_index = prepared.edge_index();
    let mut events = BTreeMap::<i64, SweepEventBucket>::new();
    for edge_id in 0..edge_index.edge_count() {
        let edge = edge_index.edge(edge_id).expect("edge identity is in range");
        let is_status_segment = match axis {
            SweepAxis::Horizontal => !edge.is_horizontal(),
            SweepAxis::Vertical => edge.is_horizontal(),
        };
        if !is_status_segment {
            continue;
        }
        let (start, end) = match axis {
            SweepAxis::Horizontal => (edge.bottom(), edge.top()),
            SweepAxis::Vertical => (edge.left(), edge.right()),
        };
        events.entry(start).or_default().insertions.push(edge_id);
        events.entry(end).or_default().removals.push(edge_id);
    }
    let reflex_points = boundary
        .reflex_vertices
        .iter()
        .map(|vertex| vertex.point)
        .collect::<BTreeSet<_>>();
    for &point in &reflex_points {
        let vertex_id = boundary_index
            .vertex_id(point)
            .ok_or(PolygonSgError::EndpointNotOnBoundary { point })?;
        let coordinate = match axis {
            SweepAxis::Horizontal => point.y,
            SweepAxis::Vertical => point.x,
        };
        events
            .entry(coordinate)
            .or_default()
            .queries
            .push((point, vertex_id));
    }

    let mut status = BTreeSet::<(i64, usize)>::new();
    let mut outputs = BTreeSet::<(i64, i64, i64)>::new();
    let mut certificate = SweepCertificate::default();
    for (coordinate, mut bucket) in events {
        bucket.insertions.sort_unstable();
        bucket.removals.sort_unstable();
        bucket.queries.sort_unstable_by_key(|&(point, vertex_id)| {
            (
                match axis {
                    SweepAxis::Horizontal => point.x,
                    SweepAxis::Vertical => point.y,
                },
                vertex_id,
            )
        });
        record_sweep_events(
            metrics,
            axis,
            bucket.insertions.len() + bucket.queries.len() + bucket.removals.len(),
        );
        for edge_id in &bucket.insertions {
            let edge = edge_index
                .edge(*edge_id)
                .expect("edge identity is in range");
            let transverse = match axis {
                SweepAxis::Horizontal => edge.first.x,
                SweepAxis::Vertical => edge.first.y,
            };
            status.insert((transverse, *edge_id));
            metrics.sweep_status_insertions += 1;
            metrics.sweep_auxiliary_tree_operations += 1;
        }
        for &(source_point, source) in &bucket.queries {
            metrics.sweep_status_queries += 1;
            metrics.sweep_auxiliary_tree_operations += 1;
            let transverse = match axis {
                SweepAxis::Horizontal => source_point.x,
                SweepAxis::Vertical => source_point.y,
            };
            let increasing = sweep_interior_direction(boundary, source, axis)?;
            let blocker = if increasing {
                status
                    .range((Excluded((transverse, usize::MAX)), Unbounded))
                    .next()
                    .copied()
            } else {
                status
                    .range((Unbounded, Excluded((transverse, usize::MIN))))
                    .next_back()
                    .copied()
            };
            let Some((_, blocker_edge_id)) = blocker else {
                continue;
            };
            let blocker_edge = edge_index
                .edge(blocker_edge_id)
                .expect("status edge identity is in range");
            let target_point = match axis {
                SweepAxis::Horizontal => Point::new(blocker_edge.first.x, coordinate),
                SweepAxis::Vertical => Point::new(coordinate, blocker_edge.first.y),
            };
            let Some(target) = boundary_index.vertex_id(target_point) else {
                continue;
            };
            if !reflex_points.contains(&target_point) || source_point >= target_point {
                continue;
            }
            let key = match axis {
                SweepAxis::Horizontal => (coordinate, source_point.x, target_point.x),
                SweepAxis::Vertical => (coordinate, source_point.y, target_point.y),
            };
            if !outputs.insert(key) {
                metrics.sweep_duplicate_output_count += 1;
                continue;
            }
            match axis {
                SweepAxis::Horizontal => metrics.sweep_output_horizontal_chords += 1,
                SweepAxis::Vertical => metrics.sweep_output_vertical_chords += 1,
            }
            certificate.output_records.push(SweepOutputRecord {
                axis,
                source,
                target,
                source_point,
                target_point,
                blocker_edge_id,
            });
        }
        for edge_id in &bucket.removals {
            let edge = edge_index
                .edge(*edge_id)
                .expect("edge identity is in range");
            let transverse = match axis {
                SweepAxis::Horizontal => edge.first.x,
                SweepAxis::Vertical => edge.first.y,
            };
            status.remove(&(transverse, *edge_id));
            metrics.sweep_status_deletions += 1;
            metrics.sweep_auxiliary_tree_operations += 1;
        }
        if certificate.event_summaries.len() < SWEEP_EVENT_TRACE_LIMIT {
            certificate.event_summaries.push(SweepEventSummary {
                axis,
                coordinate,
                inserted_segment_count: bucket.insertions.len(),
                query_count: bucket.queries.len(),
                removed_segment_count: bucket.removals.len(),
                insert_query_remove_order: true,
            });
        } else {
            certificate.event_trace_truncated = true;
        }
    }
    Ok((outputs, certificate))
}

fn record_sweep_events(
    metrics: &mut PolygonChordEnumerationMetrics,
    axis: SweepAxis,
    count: usize,
) {
    match axis {
        SweepAxis::Horizontal => metrics.sweep_horizontal_event_count += count,
        SweepAxis::Vertical => metrics.sweep_vertical_event_count += count,
    }
}

pub(crate) fn sweep_interior_direction(
    boundary: &Boundary,
    vertex_id: BoundaryVertexId,
    axis: SweepAxis,
) -> Result<bool, PolygonSgError> {
    let (previous, current, next) = incident_vertices(boundary, vertex_id)?;
    let neighbor = match axis {
        SweepAxis::Horizontal if previous.y == current.y => previous,
        SweepAxis::Horizontal if next.y == current.y => next,
        SweepAxis::Vertical if previous.x == current.x => previous,
        SweepAxis::Vertical if next.x == current.x => next,
        _ => return Err(PolygonSgError::InvalidBoundaryVertexId(vertex_id)),
    };
    Ok(match axis {
        SweepAxis::Horizontal => neighbor.x < current.x,
        SweepAxis::Vertical => neighbor.y < current.y,
    })
}
