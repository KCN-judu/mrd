//! Sparse planar subdivision and slab validation for polygon completion.
//!
//! This module deliberately never materializes the Cartesian product of the
//! x and y coordinate sets.  The coordinate arrangement in
//! `polygon_arrangement` remains the independent dense oracle.

pub mod recovery;
pub mod subdivision;
pub mod validation;
pub mod validator;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rect_core::{
        CoordinateRect, OrthogonalLoop, Point, PreparedPolygonContext, RectilinearPolygon,
    };

    use crate::polygon::{HorizontalCutSegment, VerticalCutSegment};
    use crate::polygon_arrangement;

    use super::{subdivision, validator};

    #[test]
    fn sparse_subdivision_recovers_single_rectangle_without_dense_cells() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 3),
                Point::new(0, 3),
            ]),
            vec![],
        )
        .unwrap();
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let subdivision =
            subdivision::Graph::new(&prepared, &BTreeSet::new(), &BTreeSet::new()).unwrap();
        assert_eq!(
            subdivision.recover_rectangles(&polygon).unwrap(),
            vec![CoordinateRect::new(0, 0, 4, 3).unwrap()]
        );
        validator::Validator
            .validate(&polygon, &[CoordinateRect::new(0, 0, 4, 3).unwrap()])
            .unwrap();
    }

    #[test]
    fn event_validator_matches_reference_and_performs_no_slab_rescans() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 4),
                Point::new(0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let valid = [
            CoordinateRect::new(0, 0, 2, 4).unwrap(),
            CoordinateRect::new(2, 0, 4, 4).unwrap(),
        ];
        let reference = validator::Validator
            .validate_with_backend(&polygon, &valid, validator::Backend::Oracle)
            .unwrap();
        let event = validator::Validator
            .validate_with_backend(&polygon, &valid, validator::Backend::Experiment)
            .unwrap();
        assert!(reference.boundary_edge_scans > 0);
        assert!(reference.active_rectangle_resorts > 0);
        assert_eq!(event.boundary_edge_scans, 0);
        assert_eq!(event.active_rectangle_resorts, 0);
        assert!(event.segment_tree_node_visits > 0);
        assert!(event.root_checks > 0);

        let invalid = [
            vec![CoordinateRect {
                x0: 0,
                y0: 0,
                x1: 0,
                y1: 4,
            }],
            vec![CoordinateRect::new(0, 0, 3, 4).unwrap()],
            vec![
                CoordinateRect::new(0, 0, 3, 4).unwrap(),
                CoordinateRect::new(2, 0, 3, 4).unwrap(),
            ],
            vec![
                CoordinateRect::new(0, 0, 3, 4).unwrap(),
                CoordinateRect::new(3, 0, 5, 2).unwrap(),
            ],
        ];
        for rectangles in invalid {
            let reference = validator::Validator
                .validate_with_backend(&polygon, &rectangles, validator::Backend::Oracle)
                .unwrap_err();
            let event = validator::Validator
                .validate_with_backend(&polygon, &rectangles, validator::Backend::Experiment)
                .unwrap_err();
            assert_eq!(
                std::mem::discriminant(&reference),
                std::mem::discriminant(&event),
                "reference={reference:?}, event={event:?}"
            );
        }
    }

    #[test]
    fn sparse_subdivision_matches_dense_on_t_junction_and_crossing_cuts() {
        let l_shape = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 1),
                Point::new(1, 1),
                Point::new(1, 4),
                Point::new(0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let rectangle = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 4),
                Point::new(0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let cases = [
            (
                l_shape,
                BTreeSet::new(),
                BTreeSet::from([VerticalCutSegment::new(1, 0, 1).unwrap()]),
            ),
            (
                rectangle,
                BTreeSet::from([HorizontalCutSegment::new(0, 4, 2).unwrap()]),
                BTreeSet::from([VerticalCutSegment::new(2, 0, 4).unwrap()]),
            ),
        ];
        for (polygon, horizontal, vertical) in cases {
            let prepared = PreparedPolygonContext::new(&polygon).unwrap();
            let sparse = subdivision::Graph::new(&prepared, &horizontal, &vertical)
                .unwrap()
                .recover_rectangles(&polygon)
                .unwrap();
            let mut dense =
                polygon_arrangement::Arrangement::new(&prepared, &horizontal, &vertical).unwrap();
            let dense = dense.recover_rectangles().unwrap();
            assert_eq!(sparse, dense);
            validator::Validator.validate(&polygon, &sparse).unwrap();
        }
    }

    #[test]
    fn orthogonal_sweep_matches_range_scan_junctions_and_atomic_segments() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(8, 0),
                Point::new(8, 8),
                Point::new(0, 8),
            ]),
            vec![],
        )
        .unwrap();
        let horizontal = BTreeSet::from([
            HorizontalCutSegment::new(0, 8, 2).unwrap(),
            HorizontalCutSegment::new(0, 4, 4).unwrap(),
            HorizontalCutSegment::new(2, 8, 6).unwrap(),
        ]);
        let vertical = BTreeSet::from([
            VerticalCutSegment::new(2, 0, 6).unwrap(),
            VerticalCutSegment::new(4, 2, 8).unwrap(),
            VerticalCutSegment::new(8, 0, 8).unwrap(),
        ]);
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let reference = subdivision::Graph::with_backend(
            &prepared,
            &horizontal,
            &vertical,
            subdivision::Backend::Oracle,
        )
        .unwrap();
        let sweep = subdivision::Graph::with_backend(
            &prepared,
            &horizontal,
            &vertical,
            subdivision::Backend::Experiment,
        )
        .unwrap();
        assert_eq!(reference.split_junctions, sweep.split_junctions);
        assert_eq!(reference.atomic_segments, sweep.atomic_segments);
        assert_eq!(reference.vertices, sweep.vertices);
        assert_eq!(reference.half_edges, sweep.half_edges);
        assert_eq!(reference.faces, sweep.faces);
        assert_eq!(sweep.metrics.candidate_pair_tests, 0);
        assert!(sweep.metrics.t_junction_count > 0);
        assert!(sweep.metrics.endpoint_contact_count > 0);
        assert_eq!(
            reference
                .recover_rectangles(&polygon)
                .map_err(|error| error.to_string()),
            sweep
                .recover_rectangles(&polygon)
                .map_err(|error| error.to_string())
        );
    }

    #[test]
    fn sparse_subdivision_accepts_all_ordinary_hole_bridge_topologies() {
        let outer = || {
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(20, 0),
                Point::new(20, 20),
                Point::new(0, 20),
            ])
        };
        let clockwise_rectangle = |left, bottom, right, top| {
            OrthogonalLoop::new(vec![
                Point::new(left, bottom),
                Point::new(left, top),
                Point::new(right, top),
                Point::new(right, bottom),
            ])
        };
        let cases = [
            (
                "same-boundary-component",
                RectilinearPolygon::new(outer(), vec![]).unwrap(),
                BTreeSet::new(),
                BTreeSet::from([VerticalCutSegment::new(10, 0, 20).unwrap()]),
            ),
            (
                "outer-to-hole",
                RectilinearPolygon::new(outer(), vec![clockwise_rectangle(6, 6, 10, 10)]).unwrap(),
                BTreeSet::from([HorizontalCutSegment::new(0, 6, 8).unwrap()]),
                BTreeSet::new(),
            ),
            (
                "hole-to-hole",
                RectilinearPolygon::new(
                    outer(),
                    vec![
                        clockwise_rectangle(3, 6, 7, 10),
                        clockwise_rectangle(13, 6, 17, 10),
                    ],
                )
                .unwrap(),
                BTreeSet::from([HorizontalCutSegment::new(7, 13, 8).unwrap()]),
                BTreeSet::new(),
            ),
        ];
        for (name, polygon, horizontal, vertical) in cases {
            let prepared = PreparedPolygonContext::new(&polygon).unwrap();
            let subdivision = subdivision::Graph::new(&prepared, &horizontal, &vertical)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(subdivision.metrics.vertex_count > 0, "{name}");
            assert!(subdivision.metrics.half_edge_count > 0, "{name}");
            assert!(!subdivision.faces.is_empty(), "{name}");
        }
    }
}
