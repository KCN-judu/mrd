//! Definition-level vertical slab rescan validator.

use std::collections::{BTreeMap, BTreeSet};

use mrd_domain::{CoordinateRect, DoubledPoint, MemoryEstimate, Point, RectilinearPolygon};

use crate::polygon::PolygonValidationError;

use super::{Backend, Metrics};

/// Validates by rebuilding polygon and rectangle intervals for every slab.
///
/// # Errors
///
/// Returns the first exact geometry, coverage, or area error.
#[allow(clippy::too_many_lines)]
pub fn validate(
    polygon: &RectilinearPolygon,
    rectangles: &[CoordinateRect],
) -> Result<Metrics, PolygonValidationError> {
    let mut rectangle_area = 0_i128;
    let mut x_coordinates = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
        .collect::<BTreeSet<_>>();
    let mut events = BTreeMap::<i64, Vec<(bool, usize)>>::new();
    for (index, rectangle) in rectangles.iter().copied().enumerate() {
        if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
            return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
        }
        rectangle_area = rectangle_area
            .checked_add(rectangle.area())
            .ok_or(PolygonValidationError::AreaOverflow)?;
        x_coordinates.extend([rectangle.x0, rectangle.x1]);
        events.entry(rectangle.x0).or_default().push((true, index));
        events.entry(rectangle.x1).or_default().push((false, index));
    }
    let polygon_area_twice = polygon
        .twice_signed_area()
        .map_err(PolygonValidationError::Polygon)?;
    if rectangle_area
        .checked_mul(2)
        .ok_or(PolygonValidationError::AreaOverflow)?
        != polygon_area_twice
    {
        return Err(PolygonValidationError::AreaMismatch {
            polygon_area_twice,
            rectangle_area_twice: rectangle_area * 2,
        });
    }
    let x_coordinates = x_coordinates.into_iter().collect::<Vec<_>>();
    let horizontal_boundary = polygon
        .loops()
        .flat_map(mrd_domain::OrthogonalLoop::edges)
        .filter(|(first, second)| first.y == second.y)
        .collect::<Vec<_>>();
    let mut active = BTreeSet::new();
    let mut metrics = Metrics {
        validator_backend: Backend::Oracle.name().to_owned(),
        x_event_count: x_coordinates.len(),
        owned_bytes: x_coordinates.len() * std::mem::size_of::<i64>()
            + horizontal_boundary.len() * std::mem::size_of::<(Point, Point)>(),
        ..Metrics::default()
    };
    metrics.memory_estimate = MemoryEstimate {
        retained_payload_bytes: x_coordinates.len() * std::mem::size_of::<i64>()
            + horizontal_boundary.len() * std::mem::size_of::<(Point, Point)>(),
        retained_collection_capacity_bytes: (x_coordinates.capacity() - x_coordinates.len())
            * std::mem::size_of::<i64>()
            + (horizontal_boundary.capacity() - horizontal_boundary.len())
                * std::mem::size_of::<(Point, Point)>(),
        retained_container_estimate: events.len() * std::mem::size_of::<Vec<(bool, usize)>>(),
        peak_temporary_payload_bytes: 0,
        unmeasured_allocator_overhead: true,
    };
    metrics.owned_bytes = metrics.memory_estimate.retained_total_estimate();
    for pair in x_coordinates.windows(2) {
        let x = pair[0];
        if let Some(changes) = events.get(&x) {
            for &(starts, index) in changes {
                if starts {
                    active.insert(index);
                } else {
                    active.remove(&index);
                }
            }
        }
        if pair[0] == pair[1] {
            continue;
        }
        metrics.slab_count += 1;
        metrics.boundary_edge_scans += horizontal_boundary.len();
        metrics.active_rectangle_resorts += 1;
        metrics.root_checks += 1;
        let doubled_x = i128::from(pair[0]) + i128::from(pair[1]);
        let mut crossings = horizontal_boundary
            .iter()
            .filter_map(|&(first, second)| {
                let left = 2 * i128::from(first.x.min(second.x));
                let right = 2 * i128::from(first.x.max(second.x));
                (left < doubled_x && doubled_x < right).then_some(first.y)
            })
            .collect::<Vec<_>>();
        crossings.sort_unstable();
        crossings.dedup();
        metrics.polygon_interval_events += crossings.len();
        let expected = crossings
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect::<Vec<_>>();
        let mut coverage = active
            .iter()
            .copied()
            .map(|index| {
                let rectangle = rectangles[index];
                (rectangle.y0, rectangle.y1, index)
            })
            .collect::<Vec<_>>();
        coverage.sort_unstable();
        metrics.rectangle_interval_events += coverage.len();
        let unions = rectangle_unions(&coverage, doubled_x)?;
        compare_slab_intervals(&expected, &unions, doubled_x)?;
    }
    Ok(metrics)
}

fn rectangle_unions(
    coverage: &[(i64, i64, usize)],
    doubled_x: i128,
) -> Result<Vec<(i64, i64, usize)>, PolygonValidationError> {
    let mut unions = Vec::<(i64, i64, usize)>::new();
    for &(bottom, top, index) in coverage {
        if let Some(last) = unions.last_mut() {
            if bottom < last.1 {
                return Err(PolygonValidationError::Overlap {
                    first: last.2,
                    second: index,
                    point: DoubledPoint::new(
                        doubled_x,
                        i128::from(bottom) + i128::from(last.1.min(top)),
                    ),
                });
            }
            if bottom == last.1 {
                last.1 = top;
                continue;
            }
        }
        unions.push((bottom, top, index));
    }
    Ok(unions)
}

fn compare_slab_intervals(
    expected: &[(i64, i64)],
    actual: &[(i64, i64, usize)],
    doubled_x: i128,
) -> Result<(), PolygonValidationError> {
    let mut expected_index = 0;
    let mut actual_index = 0;
    let mut expected_start = expected.first().map(|interval| interval.0);
    let mut actual_start = actual.first().map(|interval| interval.0);
    while let (Some(polygon_bottom), Some(rectangle_bottom)) = (expected_start, actual_start) {
        let polygon_top = expected[expected_index].1;
        let (rectangle_top, rectangle_id) = (actual[actual_index].1, actual[actual_index].2);
        if rectangle_bottom < polygon_bottom {
            return Err(PolygonValidationError::OutsidePolygon {
                rectangle: rectangle_id,
                point: DoubledPoint::new(
                    doubled_x,
                    i128::from(rectangle_bottom) + i128::from(rectangle_top.min(polygon_bottom)),
                ),
            });
        }
        if polygon_bottom < rectangle_bottom {
            return Err(PolygonValidationError::UncoveredInterior {
                point: DoubledPoint::new(
                    doubled_x,
                    i128::from(polygon_bottom) + i128::from(polygon_top.min(rectangle_bottom)),
                ),
            });
        }
        match polygon_top.cmp(&rectangle_top) {
            std::cmp::Ordering::Equal => {
                expected_index += 1;
                actual_index += 1;
                expected_start = expected.get(expected_index).map(|interval| interval.0);
                actual_start = actual.get(actual_index).map(|interval| interval.0);
            }
            std::cmp::Ordering::Less => {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: rectangle_id,
                    point: DoubledPoint::new(
                        doubled_x,
                        i128::from(polygon_top) + i128::from(rectangle_top),
                    ),
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(PolygonValidationError::UncoveredInterior {
                    point: DoubledPoint::new(
                        doubled_x,
                        i128::from(rectangle_top) + i128::from(polygon_top),
                    ),
                });
            }
        }
    }
    if let Some(&(bottom, top)) = expected.get(expected_index) {
        return Err(PolygonValidationError::UncoveredInterior {
            point: DoubledPoint::new(doubled_x, i128::from(bottom) + i128::from(top)),
        });
    }
    if let Some(&(bottom, top, rectangle)) = actual.get(actual_index) {
        return Err(PolygonValidationError::OutsidePolygon {
            rectangle,
            point: DoubledPoint::new(doubled_x, i128::from(bottom) + i128::from(top)),
        });
    }
    Ok(())
}
