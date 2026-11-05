//! Properties of the tool geometry over many generated inputs: whatever a hand does, the result
//! is a valid path or a refusal, never a panic and never a corrupt shape.
#![allow(clippy::expect_used)]

use emath::{Pos2, Rect, Vec2};
use flare_core::{Rgba8, ShapeId};
use flare_document::{PathSegment, Shape};
use flare_input::{InputSample, PointerKind, PressureCurve};
use flare_tools::ops::{Align, align_deltas, distribute_deltas, polygon_contains};
use flare_tools::{brush, pencil};
use proptest::prelude::*;

fn coordinate() -> impl Strategy<Value = f32> {
    -5000.0_f32..5000.0
}

fn point() -> impl Strategy<Value = Pos2> {
    (coordinate(), coordinate()).prop_map(|(x, y)| Pos2::new(x, y))
}

fn rect() -> impl Strategy<Value = Rect> {
    (point(), 1.0_f32..800.0, 1.0_f32..800.0)
        .prop_map(|(min, w, h)| Rect::from_min_size(min, Vec2::new(w, h)))
}

proptest! {
    #[test]
    fn simplifying_keeps_a_subsequence_that_starts_and_ends_where_the_drag_did(
        points in prop::collection::vec(point(), 2..200), tolerance in 0.0_f32..50.0,
    ) {
        let kept = pencil::simplify(&points, tolerance);
        prop_assert!(kept.len() >= 2 && kept.len() <= points.len());
        prop_assert_eq!(kept.first(), points.first());
        prop_assert_eq!(kept.last(), points.last());
        let mut rest = points.iter();
        for k in &kept {
            prop_assert!(rest.any(|p| p == k), "a kept point is one of the originals, in order");
        }
    }

    #[test]
    fn a_smoothed_line_is_a_valid_shape_through_the_ends(
        points in prop::collection::vec(point(), 2..80),
    ) {
        let path = pencil::smooth_path(&points).expect("two or more finite points");
        let shape = Shape::new(ShapeId::new(), path.clone(), Rgba8::TRANSPARENT);
        prop_assert!(shape.is_ok());
        let PathSegment::MoveTo(first) = path[0] else { panic!("starts with a move") };
        prop_assert!((first.as_vec().x - f64::from(points[0].x)).abs() < 1e-3);
        let end = match path.last().expect("segments") {
            PathSegment::LineTo(p) | PathSegment::CubicTo(_, _, p) => p.as_vec(),
            other => panic!("unexpected end {other:?}"),
        };
        let last = points.last().expect("points");
        prop_assert!((end.x - f64::from(last.x)).abs() < 1e-3 && (end.y - f64::from(last.y)).abs() < 1e-3);
    }

    #[test]
    fn a_brush_outline_is_a_closed_valid_shape_or_nothing_for_any_pressure(
        raw in prop::collection::vec((coordinate(), coordinate(), 0.0_f32..=1.0), 0..120),
        width in 0.1_f64..300.0, min in 0.0_f32..1.0, tolerance in 0.0_f64..20.0,
    ) {
        let samples: Vec<InputSample> = raw
            .iter()
            .filter_map(|(x, y, p)| InputSample::new(f64::from(*x), f64::from(*y), *p, PointerKind::Pen))
            .collect();
        match brush::outline(&samples, width, PressureCurve::new(min, 1.0), tolerance) {
            None => {}
            Some(path) => {
                prop_assert!(matches!(path.last(), Some(PathSegment::Close)));
                prop_assert!(Shape::new(ShapeId::new(), path, Rgba8::opaque(0, 0, 0)).is_ok());
            }
        }
    }

    #[test]
    fn aligning_puts_every_shape_on_the_same_line_whatever_they_were(
        rects in prop::collection::vec(rect(), 1..12), target in rect(),
    ) {
        for how in [Align::Left, Align::CenterH, Align::Right, Align::Top, Align::CenterV, Align::Bottom] {
            let moved: Vec<Rect> = rects
                .iter()
                .zip(align_deltas(&rects, how, target))
                .map(|(r, d)| r.translate(d))
                .collect();
            for r in &moved {
                let (got, want) = match how {
                    Align::Left => (r.left(), target.left()),
                    Align::CenterH => (r.center().x, target.center().x),
                    Align::Right => (r.right(), target.right()),
                    Align::Top => (r.top(), target.top()),
                    Align::CenterV => (r.center().y, target.center().y),
                    Align::Bottom => (r.bottom(), target.bottom()),
                };
                prop_assert!((got - want).abs() < 0.01, "{how:?}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn distributing_leaves_the_outer_two_and_equal_gaps_between_the_rest(
        rects in prop::collection::vec(rect(), 0..10), horizontal in any::<bool>(),
    ) {
        let deltas = distribute_deltas(&rects, horizontal);
        prop_assert_eq!(deltas.len(), rects.len());
        if rects.len() < 3 {
            prop_assert!(deltas.iter().all(|d| *d == Vec2::ZERO));
            return Ok(());
        }
        let axis = |r: &Rect| if horizontal { (r.left(), r.right()) } else { (r.top(), r.bottom()) };
        // Nothing moves across the other axis.
        for d in &deltas {
            let off_axis = if horizontal { d.y } else { d.x };
            prop_assert_eq!(off_axis, 0.0);
        }
        let first = rects.iter().map(|r| axis(r).0).fold(f32::MAX, f32::min);
        let last = rects.iter().map(|r| axis(r).1).fold(f32::MIN, f32::max);
        let total: f32 = rects.iter().map(|r| axis(r).1 - axis(r).0).sum();
        let slack = last - first - total;
        let moved: Vec<(f32, f32)> =
            rects.iter().zip(&deltas).map(|(r, d)| axis(&r.translate(*d))).collect();
        if slack > 1.0 {
            // Room to spare: equal gaps between neighbours.
            let mut sorted = moved.clone();
            sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
            let gaps: Vec<f32> = sorted.windows(2).map(|w| w[1].0 - w[0].1).collect();
            for g in &gaps {
                prop_assert!((g - gaps[0]).abs() < 0.05, "gaps {gaps:?}");
            }
        } else if slack < -1.0 {
            // Too big for that: centers are evenly spaced, in the order they were in.
            let center = |a: (f32, f32)| (a.0 + a.1) / 2.0;
            let mut order: Vec<usize> = (0..rects.len()).collect();
            order.sort_by(|a, b| center(axis(&rects[*a])).total_cmp(&center(axis(&rects[*b]))));
            let centers: Vec<f32> = order.iter().map(|i| center(moved[*i])).collect();
            let step = centers[1] - centers[0];
            for w in centers.windows(2) {
                prop_assert!((w[1] - w[0] - step).abs() < 0.05, "centers {centers:?}");
            }
            prop_assert!(step >= -0.05, "the order is kept");
        }
    }

    #[test]
    fn a_point_is_in_an_axis_aligned_box_polygon_exactly_when_it_is_in_the_box(
        r in rect(), p in point(),
    ) {
        let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
        let inside = p.x > r.left() && p.x < r.right() && p.y > r.top() && p.y < r.bottom();
        let outside = p.x < r.left() || p.x > r.right() || p.y < r.top() || p.y > r.bottom();
        let answer = polygon_contains(&corners, p);
        if inside { prop_assert!(answer); }
        if outside { prop_assert!(!answer); }
    }
}

mod bones {
    use std::num::NonZeroU32;

    use flare_core::{
        DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
    };
    use flare_document::{
        Content, Document, FrameSpan, Instance, Layer, Scene, Shape, Symbol, Timeline,
    };
    use flare_history::EditorSession;
    use flare_tools::bones::{self, Bond};
    use glamx::{DAffine2, DVec2};
    use proptest::prelude::*;

    /// A chain of `count` instances, 10 apart, linked by bones one after another.
    fn chain(count: usize) -> (EditorSession, SceneId, LayerId) {
        let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 2.0, 2.0, Rgba8::WHITE).expect("rect");
        let art = Layer::new(
            LayerId::new(),
            "L",
            vec![FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape))],
        )
        .expect("layer");
        let symbol = Symbol::new(SymbolId::new(), "Part", Timeline::new(vec![art]));
        let parts: Vec<Instance> = (0..count)
            .map(|i| {
                let at = DAffine2::from_translation(DVec2::new(10.0 * i as f64, 0.0));
                Instance::new(
                    InstanceId::new(),
                    symbol.id(),
                    Transform2D::new(at).expect("finite"),
                )
            })
            .collect();
        let ids: Vec<InstanceId> = parts.iter().map(Instance::id).collect();
        let mut elements = parts.into_iter().map(Content::Instance);
        let first = elements.next().expect("one");
        let span = FrameSpan::new(0, NonZeroU32::new(30).expect("30"), first)
            .with_extras(elements.collect())
            .expect("elements");
        let scene = SceneId::new();
        let layer = Layer::new(LayerId::new(), "Parts", vec![span]).expect("layer");
        let document = Document::new(
            DocumentId::new(),
            FrameRate::new(24, 1).expect("rate"),
            vec![Scene::new(scene, "S", Timeline::new(vec![layer]))],
            vec![symbol],
        )
        .expect("document");
        let mut session = EditorSession::new(document);
        for pair in ids.windows(2) {
            let bond = |instance| Bond { instance, at: None };
            let commands = bones::connect(
                session.document(),
                scene,
                0,
                bond(pair[0]),
                bond(pair[1]),
                "A",
            )
            .expect("connect");
            session.apply(commands).expect("valid");
        }
        let armature = session.document().scenes()[0].timeline().layers()[0].id();
        session.clear_history();
        (session, scene, armature)
    }

    #[derive(Debug, Clone)]
    enum Op {
        Pull(usize, u32, f64, f64),
        Turn(usize, u32, f64),
        Limit(usize, f64, f64),
        Detach(usize),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (0usize..8, 0u32..32, -200.0..200.0, -200.0..200.0)
                .prop_map(|(j, f, x, y)| Op::Pull(j, f, x, y)),
            (0usize..8, 0u32..32, -10.0..10.0).prop_map(|(j, f, t)| Op::Turn(j, f, t)),
            (0usize..8, -7.0..7.0, -7.0..7.0).prop_map(|(j, a, b)| Op::Limit(j, a, b)),
            (0usize..8).prop_map(Op::Detach),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn posing_never_breaks_the_document_and_undoes_to_the_start(
            count in 2usize..7, ops in prop::collection::vec(op(), 1..24),
        ) {
            let (mut session, scene, layer) = chain(count);
            let start = session.document().clone();
            let mut steps = 0;
            for op in ops {
                let doc = session.document();
                let made = match op {
                    Op::Pull(j, f, x, y) => bones::pose_joint(doc, scene, layer, f, j, DVec2::new(x, y)),
                    Op::Turn(j, f, t) => bones::turn_joint(doc, scene, layer, f, j, t),
                    Op::Limit(j, a, b) => bones::set_limit(doc, scene, layer, j, Some((a.min(b), a.max(b)))),
                    Op::Detach(j) => bones::detach(doc, scene, layer, j),
                };
                let Ok(commands) = made else { continue };
                let before = session.document().clone();
                match session.apply(commands) {
                    Ok(()) => steps += 1,
                    // A refused batch changes nothing.
                    Err(_) => prop_assert_eq!(session.document(), &before),
                }
                // Every pose keeps every joint inside its limits.
                for span in session.document().scenes()[0].timeline().layers()[0].spans() {
                    if let Content::Armature(a) = span.content() {
                        for (joint, posed) in a.joints().iter().zip(a.pose()) {
                            prop_assert!((joint.limited(posed.turn) - posed.turn).abs() < 1e-9);
                        }
                    }
                }
            }
            for _ in 0..steps {
                prop_assert!(session.undo());
            }
            prop_assert_eq!(session.document(), &start);
        }
    }
}
