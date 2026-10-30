//! Easing and shape tweens: what the evaluator draws between two keyframes.
#![allow(clippy::expect_used)]

use std::borrow::Cow;
use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Gradient, Instance, Layer, PathSegment, Scene, Shape, Stroke,
    Symbol, Timeline, Tween,
};
use flare_eval::{eased, evaluate_frame, morph};
use glamx::{DAffine2, DVec2};

fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

fn pt(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

fn polygon(points: &[(f64, f64)], fill: Rgba8) -> Shape {
    let mut path = vec![PathSegment::MoveTo(pt(points[0].0, points[0].1))];
    path.extend(
        points[1..]
            .iter()
            .map(|p| PathSegment::LineTo(pt(p.0, p.1))),
    );
    path.push(PathSegment::Close);
    Shape::new(ShapeId::new(), path, fill).expect("polygon")
}

fn square(x: f64, size: f64, fill: Rgba8) -> Shape {
    polygon(
        &[(x, 0.0), (x + size, 0.0), (x + size, size), (x, size)],
        fill,
    )
}

fn document(layers: Vec<Layer>, symbols: Vec<Symbol>) -> (Document, SceneId) {
    let scene = SceneId::new();
    let scene_value = Scene::new(scene, "S", Timeline::new(layers));
    let doc = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene_value],
        symbols,
    )
    .expect("document");
    (doc, scene)
}

/// A layer with a shape for `duration` frames, tweened to a second shape.
fn shape_tween(a: Shape, b: Shape, duration: u32, ease: i8) -> (Document, SceneId) {
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![
            span(0, duration, Content::Shape(a))
                .with_tween(Tween::Shape)
                .with_ease(ease),
            span(duration, 1, Content::Shape(b)),
        ],
    )
    .expect("layer");
    document(vec![layer], vec![])
}

fn corners(shape: &Shape) -> Vec<DVec2> {
    shape
        .path()
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec()),
            PathSegment::CubicTo(_, _, p) => Some(p.as_vec()),
            PathSegment::Close => None,
        })
        .collect()
}

fn bounds(shape: &Shape) -> (DVec2, DVec2) {
    let points = corners(shape);
    let min = points.iter().fold(DVec2::splat(f64::MAX), |m, p| m.min(*p));
    let max = points.iter().fold(DVec2::splat(f64::MIN), |m, p| m.max(*p));
    (min, max)
}

fn close(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-6
}

// ---- easing -----------------------------------------------------------------------------

#[test]
fn easing_keeps_the_ends_and_leans_the_way_its_sign_says() {
    for ease in [-100, -50, 0, 50, 100] {
        assert_eq!(eased(0.0, ease), 0.0, "{ease}");
        assert!((eased(1.0, ease) - 1.0).abs() < 1e-12, "{ease}");
    }
    assert_eq!(eased(0.3, 0), 0.3, "no ease is even speed");
    assert!(
        eased(0.5, 50) > 0.5,
        "a positive ease is ahead early and ends slowly"
    );
    assert!(eased(0.5, -50) < 0.5, "a negative ease starts slowly");
    assert!((eased(0.5, 100) - 0.75).abs() < 1e-12);
    assert!((eased(0.5, -100) - 0.25).abs() < 1e-12);
}

#[test]
fn easing_never_goes_backwards_stays_in_range_and_mirrors() {
    for ease in [-100, -37, 0, 37, 100] {
        let mut last = 0.0;
        for step in 0..=200 {
            let t = f64::from(step) / 200.0;
            let value = eased(t, ease);
            assert!((0.0..=1.0).contains(&value), "{ease} at {t}: {value}");
            assert!(value >= last - 1e-12, "{ease} at {t}");
            last = value;
            assert!(
                (value + eased(1.0 - t, -ease) - 1.0).abs() < 1e-12,
                "mirror at {t}"
            );
        }
    }
    assert_eq!(eased(-3.0, 40), 0.0);
    assert!((eased(7.0, 40) - 1.0).abs() < 1e-12);
    assert_eq!(
        eased(f64::NAN, 40),
        0.0,
        "a number that is not one holds the start"
    );
}

#[test]
fn a_motion_tween_follows_its_easing() {
    let symbol = SymbolId::new();
    let art = Symbol::new(
        symbol,
        "Dot",
        Timeline::new(vec![
            Layer::new(
                LayerId::new(),
                "in",
                vec![span(0, 1, Content::Shape(square(0.0, 10.0, Rgba8::WHITE)))],
            )
            .expect("layer"),
        ]),
    );
    let at = |x: f64| {
        let t = Transform2D::new(DAffine2::from_translation(DVec2::new(x, 0.0))).expect("finite");
        Content::Instance(Instance::new(InstanceId::new(), symbol, t))
    };
    let x_at = |ease: i8, frame: u32| {
        let layer = Layer::new(
            LayerId::new(),
            "L",
            vec![
                span(0, 10, at(0.0))
                    .with_tween(Tween::Linear)
                    .with_ease(ease),
                span(10, 1, at(100.0)),
            ],
        )
        .expect("layer");
        let (doc, scene) = document(vec![layer], vec![art.clone()]);
        evaluate_frame(&doc, scene, frame).expect("frame").placed[0]
            .transform
            .translation
            .x
    };
    assert!((x_at(0, 5) - 50.0).abs() < 1e-9);
    assert!((x_at(100, 5) - 75.0).abs() < 1e-9);
    assert!((x_at(-100, 5) - 25.0).abs() < 1e-9);
    assert!(x_at(100, 5) > x_at(50, 5) && x_at(50, 5) > x_at(0, 5));
}

// ---- shape tweens -----------------------------------------------------------------------

#[test]
fn a_shape_tween_is_the_shape_at_its_start_and_a_step_after_it() {
    let (doc, scene) = shape_tween(
        square(0.0, 10.0, Rgba8::opaque(255, 0, 0)),
        square(20.0, 10.0, Rgba8::opaque(0, 0, 255)),
        4,
        0,
    );
    let first = evaluate_frame(&doc, scene, 0).expect("frame");
    assert!(
        matches!(first.shapes[0].shape, Cow::Borrowed(_)),
        "the keyframe is the document's"
    );
    assert_eq!(first.shapes[0].phase, 0);
    let id = first.shapes[0].shape.id();
    let mut signatures = vec![first.signature()];
    for frame in 1..4 {
        let step = evaluate_frame(&doc, scene, frame).expect("frame");
        let drawn = &step.shapes[0];
        assert!(
            matches!(drawn.shape, Cow::Owned(_)),
            "frame {frame} is a step"
        );
        assert_eq!(drawn.shape.id(), id, "a step keeps its shape's identity");
        assert_ne!(drawn.phase, 0);
        let (min, max) = bounds(&drawn.shape);
        let expected = 5.0 * f64::from(frame);
        assert!(
            close(min, DVec2::new(expected, 0.0)),
            "frame {frame}: {min:?}"
        );
        assert!(
            close(max, DVec2::new(expected + 10.0, 10.0)),
            "frame {frame}: {max:?}"
        );
        signatures.push(step.signature());
    }
    signatures.sort_unstable();
    signatures.dedup();
    assert_eq!(
        signatures.len(),
        4,
        "every frame is its own picture, though the id is shared"
    );
}

#[test]
fn a_shape_tween_blends_the_fill_and_a_shape_without_one_holds() {
    let (doc, scene) = shape_tween(
        square(0.0, 10.0, Rgba8::opaque(200, 0, 0)),
        square(0.0, 10.0, Rgba8::opaque(0, 100, 200)),
        2,
        0,
    );
    let middle = evaluate_frame(&doc, scene, 1).expect("frame");
    assert_eq!(middle.shapes[0].shape.fill(), Rgba8::opaque(100, 50, 100));
    // No tween: the span is held, whatever follows it.
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![
            span(0, 2, Content::Shape(square(0.0, 10.0, Rgba8::WHITE))),
            span(2, 1, Content::Shape(square(50.0, 10.0, Rgba8::WHITE))),
        ],
    )
    .expect("layer");
    let (held, scene) = document(vec![layer], vec![]);
    let frame = evaluate_frame(&held, scene, 1).expect("frame");
    assert!(matches!(frame.shapes[0].shape, Cow::Borrowed(_)));
    // A tween toward something that is not a shape holds too.
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![
            span(0, 2, Content::Shape(square(0.0, 10.0, Rgba8::WHITE))).with_tween(Tween::Shape),
            span(2, 1, Content::Empty),
        ],
    )
    .expect("layer");
    let (toward_nothing, scene) = document(vec![layer], vec![]);
    let frame = evaluate_frame(&toward_nothing, scene, 1).expect("frame");
    assert!(matches!(frame.shapes[0].shape, Cow::Borrowed(_)));
}

#[test]
fn a_shape_tween_follows_its_easing() {
    let mid = |ease: i8| {
        let (doc, scene) = shape_tween(
            square(0.0, 10.0, Rgba8::WHITE),
            square(100.0, 10.0, Rgba8::WHITE),
            10,
            ease,
        );
        let frame = evaluate_frame(&doc, scene, 5).expect("frame");
        bounds(&frame.shapes[0].shape).0.x
    };
    assert!((mid(0) - 50.0).abs() < 1e-9);
    assert!((mid(100) - 75.0).abs() < 1e-9);
    assert!((mid(-100) - 25.0).abs() < 1e-9);
}

#[test]
fn shapes_with_different_numbers_of_corners_meet_at_their_own_ends() {
    let a = square(0.0, 10.0, Rgba8::WHITE);
    let b = polygon(&[(50.0, 0.0), (60.0, 0.0), (55.0, 10.0)], Rgba8::WHITE);
    let start = morph(&a, &b, 0.0).expect("morph");
    let end = morph(&a, &b, 1.0).expect("morph");
    let has = |shape: &Shape, p: DVec2| corners(shape).iter().any(|c| close(*c, p));
    for corner in corners(&a) {
        assert!(has(&start, corner), "the start keeps {corner:?}");
    }
    for corner in corners(&b) {
        assert!(has(&end, corner), "the end reaches {corner:?}");
    }
    let (min, max) = bounds(&start);
    assert!(close(min, DVec2::new(0.0, 0.0)) && close(max, DVec2::new(10.0, 10.0)));
    let (min, max) = bounds(&end);
    assert!(close(min, DVec2::new(50.0, 0.0)) && close(max, DVec2::new(60.0, 10.0)));
}

#[test]
fn a_square_that_winds_the_other_way_or_starts_at_another_corner_does_not_twist() {
    let a = square(0.0, 10.0, Rgba8::WHITE);
    let backwards = polygon(
        &[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)],
        Rgba8::WHITE,
    );
    let from_another_corner = polygon(
        &[(10.0, 10.0), (0.0, 10.0), (0.0, 0.0), (10.0, 0.0)],
        Rgba8::WHITE,
    );
    for other in [&backwards, &from_another_corner] {
        for t in [0.25, 0.5, 0.75] {
            let step = morph(&a, other, t).expect("morph");
            let (min, max) = bounds(&step);
            assert!(close(min, DVec2::new(0.0, 0.0)), "{t}: {min:?}");
            assert!(close(max, DVec2::new(10.0, 10.0)), "{t}: {max:?}");
        }
    }
}

#[test]
fn a_contour_with_no_partner_shrinks_to_its_middle_and_grows_from_it() {
    let ring = {
        let mut path = square(0.0, 40.0, Rgba8::WHITE).path().to_vec();
        path.extend([
            PathSegment::MoveTo(pt(10.0, 10.0)),
            PathSegment::LineTo(pt(10.0, 30.0)),
            PathSegment::LineTo(pt(30.0, 30.0)),
            PathSegment::LineTo(pt(30.0, 10.0)),
            PathSegment::Close,
        ]);
        Shape::new(ShapeId::new(), path, Rgba8::WHITE).expect("ring")
    };
    let solid = square(0.0, 40.0, Rgba8::WHITE);
    let moves = |s: &Shape| {
        s.path()
            .iter()
            .filter(|seg| matches!(seg, PathSegment::MoveTo(_)))
            .count()
    };
    let middle = morph(&ring, &solid, 0.5).expect("morph");
    assert_eq!(moves(&middle), 2, "the hole is still there, smaller");
    let end = morph(&ring, &solid, 1.0).expect("morph");
    let hole_points: Vec<DVec2> = corners(&end)[corners(&end).len() - 4..].to_vec();
    let spread = hole_points
        .iter()
        .fold(0.0_f64, |m, p| m.max((*p - hole_points[0]).length()));
    assert!(spread < 1e-6, "by the end the hole has closed to a point");
    let back = morph(&solid, &ring, 0.0).expect("morph");
    assert_eq!(moves(&back), 2);
}

#[test]
fn colors_gradients_and_strokes_blend_and_a_one_sided_stroke_fades() {
    let outlined = square(0.0, 10.0, Rgba8::TRANSPARENT)
        .with_stroke(Stroke::new(2.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let plain = square(0.0, 10.0, Rgba8::TRANSPARENT);
    let fading = morph(&outlined, &plain, 0.5).expect("morph");
    let stroke = fading.stroke().expect("the stroke is still there, fading");
    assert_eq!(stroke.color().a, 128);
    let appearing = morph(&plain, &outlined, 0.25).expect("morph");
    assert_eq!(appearing.stroke().expect("stroke").color().a, 64);
    let thick = square(0.0, 10.0, Rgba8::TRANSPARENT)
        .with_stroke(Stroke::new(6.0, Rgba8::opaque(200, 0, 0)).expect("stroke"));
    let both = morph(&outlined, &thick, 0.5).expect("morph");
    let stroke = both.stroke().expect("stroke");
    assert_eq!(
        (stroke.width(), stroke.color()),
        (4.0, Rgba8::opaque(100, 0, 0))
    );
    let gradient = |end: f64, color: Rgba8| {
        square(0.0, 10.0, Rgba8::WHITE).with_gradient(
            Gradient::linear(pt(0.0, 0.0), pt(end, 0.0), Rgba8::opaque(0, 0, 0), color)
                .expect("gradient"),
        )
    };
    let blended = morph(
        &gradient(10.0, Rgba8::opaque(100, 0, 0)),
        &gradient(30.0, Rgba8::opaque(200, 0, 0)),
        0.5,
    )
    .expect("morph");
    let g = blended.gradient().expect("a gradient");
    assert_eq!(g.to().as_vec().x, 20.0);
    assert_eq!(g.end(), Rgba8::opaque(150, 0, 0));
    let one_sided = morph(&gradient(10.0, Rgba8::opaque(100, 0, 0)), &plain, 0.5).expect("morph");
    assert!(
        one_sided.gradient().is_none(),
        "a gradient against none becomes one color"
    );
}

#[test]
fn shapes_that_cannot_be_matched_are_refused_and_so_are_enormous_ones() {
    let closed = square(0.0, 10.0, Rgba8::WHITE);
    let open = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(pt(0.0, 0.0)),
            PathSegment::LineTo(pt(10.0, 0.0)),
            PathSegment::LineTo(pt(10.0, 10.0)),
        ],
        Rgba8::WHITE,
    )
    .expect("open");
    assert!(
        morph(&closed, &open, 0.5).is_none(),
        "a closed outline against an open one"
    );
    let many = {
        let mut path = Vec::new();
        for i in 0..300 {
            let x = f64::from(i) * 3.0;
            path.push(PathSegment::MoveTo(pt(x, 0.0)));
            path.push(PathSegment::LineTo(pt(x + 1.0, 0.0)));
            path.push(PathSegment::LineTo(pt(x + 1.0, 1.0)));
            path.push(PathSegment::Close);
        }
        Shape::new(ShapeId::new(), path, Rgba8::WHITE).expect("many")
    };
    assert!(
        morph(&many, &closed, 0.5).is_none(),
        "more contours than a morph takes"
    );
    // And a tween that cannot be built holds the shape instead of failing the frame.
    let (doc, scene) = shape_tween(closed, open, 4, 0);
    let frame = evaluate_frame(&doc, scene, 2).expect("the frame still evaluates");
    assert!(matches!(frame.shapes[0].shape, Cow::Borrowed(_)));
}
