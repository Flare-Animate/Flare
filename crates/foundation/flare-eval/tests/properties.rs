//! Properties of evaluation and path following over many generated inputs.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, DocumentPoint, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, PathSegment, Scene, Shape, Timeline};
use flare_eval::{evaluate_frame, point_along};
use proptest::prelude::*;

fn p(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

proptest! {
    /// A point that travels along a straight-line path is always inside the path's bounding box,
    /// starts at the first point and ends at the last.
    #[test]
    fn following_a_polyline_stays_in_its_box_and_runs_from_first_point_to_last(
        points in prop::collection::vec((-1000.0_f64..1000.0, -1000.0_f64..1000.0), 2..30),
        fraction in -0.5_f64..1.5,
    ) {
        let mut path = vec![PathSegment::MoveTo(p(points[0].0, points[0].1))];
        path.extend(points[1..].iter().map(|(x, y)| PathSegment::LineTo(p(*x, *y))));
        let at = point_along(&path, fraction).expect("a point");
        let (lo_x, hi_x) = points.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.0), a.1.max(q.0)));
        let (lo_y, hi_y) = points.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.1), a.1.max(q.1)));
        prop_assert!(at.x >= lo_x - 1e-9 && at.x <= hi_x + 1e-9 && at.y >= lo_y - 1e-9 && at.y <= hi_y + 1e-9);
        let start = point_along(&path, 0.0).expect("start");
        let end = point_along(&path, 1.0).expect("end");
        prop_assert!((start.x - points[0].0).abs() < 1e-9 && (start.y - points[0].1).abs() < 1e-9);
        let last = points.last().expect("points");
        prop_assert!((end.x - last.0).abs() < 1e-6 && (end.y - last.1).abs() < 1e-6);
    }

    /// Whatever the layers and frames, evaluation succeeds, draws one shape per visible layer
    /// with a span at the frame, and gives the same answer twice.
    #[test]
    fn evaluation_draws_exactly_the_visible_layers_that_have_something_at_the_frame(
        layers in prop::collection::vec((any::<bool>(), 0_u32..20, 1_u32..20), 1..12),
        frame in 0_u32..40,
    ) {
        let built: Vec<Layer> = layers
            .iter()
            .enumerate()
            .map(|(i, (visible, start, length))| {
                let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, Rgba8::WHITE).expect("rect");
                let span = FrameSpan::new(*start, NonZeroU32::new(*length).expect("nonzero"), Content::Shape(shape));
                Layer::new(LayerId::new(), format!("L{i}"), vec![span]).expect("layer").with_visible(*visible)
            })
            .collect();
        let expected = layers
            .iter()
            .filter(|(visible, start, length)| *visible && frame >= *start && frame < start + length)
            .count();
        let scene = SceneId::new();
        let doc = Document::new(
            DocumentId::new(),
            FrameRate::new(24, 1).expect("rate"),
            vec![Scene::new(scene, "S", Timeline::new(built))],
            vec![],
        )
        .expect("document");
        let first = evaluate_frame(&doc, scene, frame).expect("evaluation");
        prop_assert_eq!(first.shapes.len(), expected);
        let second = evaluate_frame(&doc, scene, frame).expect("evaluation");
        prop_assert_eq!(first.signature(), second.signature());
    }
}
