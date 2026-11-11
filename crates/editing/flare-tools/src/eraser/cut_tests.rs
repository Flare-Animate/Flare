#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::*;
use flare_core::{LayerId, Rgba8, ShapeId};
use flare_document::{FrameSpan, Layer, PathSegment, Shape, Stroke, TextSource};
use std::num::NonZeroU32;

/// A closed box from `x0, y0` to `x1, y1`, as an eraser outline.
fn band(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
    vec![
        DVec2::new(x0, y0),
        DVec2::new(x1, y0),
        DVec2::new(x1, y1),
        DVec2::new(x0, y1),
    ]
}

fn points(shape: &Shape) -> Vec<DVec2> {
    shape
        .path()
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec()),
            _ => None,
        })
        .collect()
}

fn moves(shape: &Shape) -> usize {
    shape
        .path()
        .iter()
        .filter(|s| matches!(s, PathSegment::MoveTo(_)))
        .count()
}

fn left(cut: Cut) -> Vec<Shape> {
    match cut {
        Cut::Left(pieces) => pieces,
        Cut::Untouched => panic!("the eraser should have cut it"),
    }
}

fn line(from: (f64, f64), to: (f64, f64)) -> Shape {
    Shape::line(
        ShapeId::new(),
        from,
        to,
        Stroke::new(2.0, RED).expect("stroke"),
    )
    .expect("line")
}

#[test]
fn a_band_across_a_fill_cuts_it_in_two_and_nothing_is_left_inside_the_band() {
    let rect = Shape::rect(ShapeId::new(), 0.0, 0.0, 100.0, 40.0, RED).expect("rect");
    let pieces = left(cut(&rect, &[band(40.0, -10.0, 60.0, 50.0)]));
    assert_eq!(pieces.len(), 1, "one fill, no outline to keep");
    let fill = &pieces[0];
    assert_eq!(moves(fill), 2, "two pieces of fill");
    assert!(fill.stroke().is_none() && fill.fill() == RED);
    assert!(
        points(fill)
            .iter()
            .all(|p| p.x <= 40.0 + 1e-6 || p.x >= 60.0 - 1e-6)
    );
}

#[test]
fn a_miss_leaves_the_shape_as_it_was() {
    let rect = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, RED).expect("rect");
    assert_eq!(cut(&rect, &[band(50.0, 50.0, 60.0, 60.0)]), Cut::Untouched);
    // Close by, inside its box but not over it: a diagonal band missing a line's ends.
    let diagonal = line((0.0, 0.0), (100.0, 100.0));
    assert_eq!(
        cut(&diagonal, &[band(60.0, 0.0, 100.0, 30.0)]),
        Cut::Untouched
    );
    assert_eq!(
        cut(&rect, &[band(0.0, 0.0, 1.0, 1.0)[..2].to_vec()]),
        Cut::Untouched
    );
}

#[test]
fn a_line_is_cut_where_the_eraser_crosses_it() {
    let pieces = left(cut(
        &line((0.0, 10.0), (100.0, 10.0)),
        &[band(30.0, 0.0, 50.0, 20.0)],
    ));
    assert_eq!(pieces.len(), 1);
    let rest = &pieces[0];
    assert_eq!(moves(rest), 2, "the two ends are left");
    assert!(rest.fill_is_invisible());
    assert_eq!(rest.stroke().map(|s| s.width()), Some(2.0));
    let xs: Vec<f64> = points(rest).iter().map(|p| p.x).collect();
    assert!(
        xs.iter().all(|x| *x <= 30.0 + 1e-6 || *x >= 50.0 - 1e-6),
        "{xs:?}"
    );
    assert!(xs.contains(&0.0) && xs.contains(&100.0));
}

#[test]
fn an_outlined_fill_keeps_its_fill_and_its_line_apart_and_the_cut_edge_has_no_outline() {
    let stroke = Stroke::new(3.0, Rgba8::opaque(0, 0, 255)).expect("stroke");
    let rect = Shape::rect(ShapeId::new(), 0.0, 0.0, 100.0, 100.0, RED)
        .expect("rect")
        .with_stroke(stroke);
    let pieces = left(cut(&rect, &[band(80.0, 80.0, 120.0, 120.0)]));
    assert_eq!(pieces.len(), 2);
    let (fill, outline) = (&pieces[0], &pieces[1]);
    assert!(fill.stroke().is_none() && fill.fill() == RED);
    assert!(outline.fill_is_invisible() && outline.stroke() == Some(stroke));
    // The corner is gone from both, and the line was not drawn along the new edge of the fill.
    assert!(
        points(fill)
            .iter()
            .all(|p| !(p.x > 80.0 + 1e-6 && p.y > 80.0 + 1e-6))
    );
    assert!(
        points(outline)
            .iter()
            .all(|p| !(p.x > 80.0 + 1e-6 && p.y > 80.0 + 1e-6))
    );
    assert!(!points(outline).iter().any(|p| (p.x - 80.0).abs() < 1e-6
        && p.y > 1.0
        && p.y < 99.0
        && (p.y - 80.0).abs() > 1e-6));
}

#[test]
fn rubbing_out_all_of_a_shape_leaves_nothing_and_text_is_left_whole() {
    let rect = Shape::rect(ShapeId::new(), 10.0, 10.0, 10.0, 10.0, RED).expect("rect");
    assert_eq!(
        cut(&rect, &[band(0.0, 0.0, 50.0, 50.0)]),
        Cut::Left(Vec::new())
    );
    let source = TextSource::new(
        "Hi",
        "sans-serif",
        12.0,
        false,
        false,
        flare_core::DocumentPoint::new(10.0, 10.0).expect("point"),
    )
    .expect("text");
    let text = rect.clone().with_text(source);
    assert_eq!(cut(&text, &[band(0.0, 0.0, 50.0, 50.0)]), Cut::Untouched);
}

#[test]
fn erasing_is_one_undo_step_across_the_layers_it_may_change() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 100.0, 40.0));
    let locked = LayerId::new();
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 100.0, 40.0, RED).expect("rect");
    let span = FrameSpan::new(0, NonZeroU32::new(5).expect("5"), Content::Shape(shape));
    f.session
        .apply([flare_commands::Command::AddLayer {
            scene: f.scene,
            layer: Layer::new(locked, "Locked", vec![span])
                .expect("layer")
                .with_locked(true),
        }])
        .expect("layer");
    let before = f.session.document().clone();
    let commands = erase(
        f.session.document(),
        f.scene,
        2,
        &[band(40.0, -10.0, 60.0, 50.0)],
    );
    assert!(!commands.is_empty());
    f.session.apply(commands).expect("erase");
    let elements = |f: &Fixture, layer: LayerId| -> Vec<Shape> {
        let timeline = f.session.document().scenes()[0].timeline();
        let layer = timeline
            .layers()
            .iter()
            .find(|l| l.id() == layer)
            .expect("layer");
        layer
            .span_at(0)
            .expect("span")
            .elements()
            .filter_map(|c| match c {
                Content::Shape(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(moves(&elements(&f, f.layer)[0]), 2, "cut in two");
    assert_eq!(
        moves(&elements(&f, locked)[0]),
        1,
        "a locked layer is not erased"
    );
    assert!(f.session.undo());
    assert_eq!(f.session.document(), &before);
    // Nothing under the eraser: no commands, so no empty undo step.
    assert!(
        erase(
            f.session.document(),
            f.scene,
            2,
            &[band(500.0, 500.0, 600.0, 600.0)]
        )
        .is_empty()
    );
}

#[test]
fn erasing_the_last_of_a_keyframe_removes_the_keyframe() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 10.0, 10.0, 10.0, 10.0));
    let commands = erase(
        f.session.document(),
        f.scene,
        0,
        &[band(0.0, 0.0, 50.0, 50.0)],
    );
    f.session.apply(commands).expect("erase");
    assert!(find_span(f.session.document(), f.scene, at(&f, 0)).is_none());
}

#[test]
fn the_eraser_covers_its_path_or_a_dot_and_refuses_a_bad_size() {
    let mouse = |x: f64, y: f64| InputSample::mouse(x, y).expect("sample");
    let dot = eraser_outline(&[mouse(5.0, 5.0)], 10.0);
    assert_eq!(dot.len(), DOT_STEPS);
    assert!(
        dot.iter()
            .all(|p| ((p - DVec2::new(5.0, 5.0)).length() - 5.0).abs() < 1e-9)
    );
    let drag = eraser_outline(
        &[mouse(0.0, 0.0), mouse(50.0, 0.0), mouse(100.0, 0.0)],
        20.0,
    );
    let (lo, hi) = drag.iter().fold((DVec2::MAX, DVec2::MIN), |(lo, hi), p| {
        (lo.min(*p), hi.max(*p))
    });
    assert!((lo.x - -10.0).abs() < 0.5 && (hi.x - 110.0).abs() < 0.5);
    assert!((lo.y - -10.0).abs() < 0.5 && (hi.y - 10.0).abs() < 0.5);
    // A light touch rubs as wide as a firm one.
    let light: Vec<InputSample> = [0.0, 50.0, 100.0]
        .iter()
        .map(|x| InputSample::new(*x, 0.0, 0.05, flare_input::PointerKind::Pen).expect("pen"))
        .collect();
    assert_eq!(eraser_outline(&light, 20.0), drag);
    assert!(eraser_outline(&[mouse(0.0, 0.0)], f64::NAN).is_empty());
    assert!(eraser_outline(&[], 10.0).is_empty());
    let huge = eraser_outline(&[mouse(0.0, 0.0)], 1.0e9);
    assert!(
        huge.iter()
            .all(|p| p.length() <= MAX_ERASER_SIZE / 2.0 + 1e-9)
    );
}

#[test]
fn a_brush_stroke_is_cut_like_any_fill() {
    let samples: Vec<InputSample> = (0..=50)
        .map(|i| InputSample::mouse(f64::from(i) * 2.0, 20.0).expect("sample"))
        .collect();
    let path =
        crate::brush::outline(&samples, 10.0, PressureCurve::default(), 0.5).expect("outline");
    let stroke = Shape::new(ShapeId::new(), path, RED).expect("shape");
    let pieces = left(cut(
        &stroke,
        &[eraser_outline(
            &[
                InputSample::mouse(50.0, 0.0).expect("s"),
                InputSample::mouse(50.0, 40.0).expect("s"),
            ],
            8.0,
        )],
    ));
    assert_eq!(moves(&pieces[0]), 2);
}

#[test]
fn several_outlines_are_rubbed_out_in_one_cut() {
    let rect = Shape::rect(ShapeId::new(), 0.0, 0.0, 100.0, 20.0, RED).expect("rect");
    let pieces = left(cut(
        &rect,
        &[band(20.0, -5.0, 30.0, 25.0), band(70.0, -5.0, 80.0, 25.0)],
    ));
    assert_eq!(moves(&pieces[0]), 3, "two cuts leave three pieces");
}
