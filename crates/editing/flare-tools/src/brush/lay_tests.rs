#![allow(clippy::expect_used)]

use flare_core::Rgba8;
use flare_document::{Content, Shape};
use flare_input::{InputSample, PointerKind, PressureCurve};

use super::*;
use crate::drawing::{DrawRefusal, Target};
use crate::test_support::*;

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn line(from: (f64, f64), to: (f64, f64), pressure: f32) -> Vec<InputSample> {
    (0..=10)
        .map(|i| {
            let t = f64::from(i) / 10.0;
            InputSample::new(
                from.0 + (to.0 - from.0) * t,
                from.1 + (to.1 - from.1) * t,
                pressure,
                PointerKind::Pen,
            )
            .expect("sample")
        })
        .collect()
}

fn click() -> Vec<InputSample> {
    vec![InputSample::new(5.0, 5.0, 1.0, PointerKind::Pen).expect("sample"); 3]
}

fn paint() -> Paint {
    Paint {
        color: BLUE,
        flow: 1.0,
        stabilizer: 0,
        tolerance: 0.5,
    }
}

fn brush(width: f64) -> BrushStroke {
    BrushStroke::new(width, PressureCurve::new(0.15, 1.0))
}

fn place(f: &Fixture) -> Placement {
    Placement {
        scene: f.scene,
        target: Target::Layer(f.layer),
        frame: 0,
        duration: None,
    }
}

fn lay(f: &mut Fixture, strokes: &[Vec<InputSample>]) -> Result<LayPlan, LayRefusal> {
    let plan = lay_strokes(
        f.session.document(),
        place(f),
        &brush(8.0),
        paint(),
        strokes,
    )?;
    f.session.apply(plan.draw.commands.clone()).expect("apply");
    Ok(plan)
}

/// The width and height of the box around a shape's points.
fn size(shape: &Shape) -> (f64, f64) {
    let points: Vec<glamx::DVec2> = shape
        .path()
        .iter()
        .filter_map(|segment| match segment {
            flare_document::PathSegment::MoveTo(p) | flare_document::PathSegment::LineTo(p) => {
                Some(p.as_vec())
            }
            _ => None,
        })
        .collect();
    let min = points
        .iter()
        .copied()
        .reduce(glamx::DVec2::min)
        .expect("points");
    let max = points
        .iter()
        .copied()
        .reduce(glamx::DVec2::max)
        .expect("points");
    (max.x - min.x, max.y - min.y)
}

fn shapes(f: &Fixture) -> Vec<Shape> {
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    let span = layer.span_at(0).expect("keyframe");
    (0..span.element_count())
        .filter_map(|i| match span.element(i) {
            Some(Content::Shape(shape)) => Some(shape.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_stroke_joins_the_keyframe_in_one_undo_step() {
    let mut f = fixture();
    let plan = lay(
        &mut f,
        &[
            line((0.0, 0.0), (100.0, 0.0), 1.0),
            line((0.0, 50.0), (100.0, 80.0), 1.0),
        ],
    )
    .expect("laid");
    assert_eq!(plan.draw.placed.len(), 2);
    assert!(plan.skipped.is_empty());
    let drawn = shapes(&f);
    assert_eq!(drawn.len(), 2);
    assert!(drawn.iter().all(|s| s.fill() == BLUE));
    assert!(f.session.undo());
    assert!(
        f.session.document().scenes()[0].timeline().layers()[0]
            .span_at(0)
            .is_none()
    );
}

#[test]
fn the_outline_is_as_wide_as_the_brush_at_full_pressure() {
    let shape = stroke_shape(
        &brush(10.0),
        &line((0.0, 0.0), (100.0, 0.0), 1.0),
        BLUE,
        1.0,
        0.5,
    )
    .expect("valid")
    .expect("drawn");
    let (width, height) = size(&shape);
    assert!((height - 10.0).abs() < 0.5, "{height}");
    assert!((width - 110.0).abs() < 0.5, "{width}");
}

#[test]
fn light_pressure_draws_a_thinner_line() {
    let height = |pressure| {
        let samples = line((0.0, 0.0), (100.0, 0.0), pressure);
        let shape = stroke_shape(&brush(10.0), &samples, BLUE, 1.0, 0.5)
            .expect("valid")
            .expect("drawn");
        size(&shape).1
    };
    assert!(height(0.3) < height(1.0) - 3.0);
}

#[test]
fn flow_makes_the_paint_see_through() {
    let shape = stroke_shape(
        &brush(10.0),
        &line((0.0, 0.0), (50.0, 0.0), 1.0),
        BLUE,
        0.5,
        0.5,
    )
    .expect("valid")
    .expect("drawn");
    assert_eq!(shape.fill().a, 127);
}

#[test]
fn a_click_is_skipped_and_named() {
    let mut f = fixture();
    let plan = lay(&mut f, &[click(), line((0.0, 0.0), (40.0, 0.0), 1.0)]).expect("laid");
    assert_eq!(plan.skipped, vec![0]);
    assert_eq!(shapes(&f).len(), 1);
}

#[test]
fn only_clicks_draw_nothing_and_change_nothing() {
    let mut f = fixture();
    let before = f.session.document().clone();
    assert_eq!(
        lay(&mut f, &[click(), click()]),
        Err(LayRefusal::NothingDrawn)
    );
    assert_eq!(f.session.document(), &before);
}

#[test]
fn no_strokes_is_refused() {
    let mut f = fixture();
    assert_eq!(lay(&mut f, &[]), Err(LayRefusal::NoStrokes));
}

#[test]
fn too_many_strokes_are_refused_before_any_is_drawn() {
    let mut f = fixture();
    let strokes = vec![line((0.0, 0.0), (10.0, 0.0), 1.0); MAX_STROKES + 1];
    assert_eq!(
        lay(&mut f, &strokes),
        Err(LayRefusal::TooManyStrokes(MAX_STROKES + 1))
    );
}

#[test]
fn a_stroke_with_too_many_points_is_refused() {
    let mut f = fixture();
    let sample = InputSample::new(0.0, 0.0, 1.0, PointerKind::Pen).expect("sample");
    let long = vec![sample; MAX_STROKE_SAMPLES + 1];
    assert_eq!(
        lay(&mut f, &[line((0.0, 0.0), (10.0, 0.0), 1.0), long]),
        Err(LayRefusal::TooManySamples {
            index: 1,
            count: MAX_STROKE_SAMPLES + 1
        })
    );
}

#[test]
fn a_locked_layer_is_refused() {
    let mut f = fixture();
    f.session
        .apply([flare_commands::Command::UpdateLayer {
            scene: f.scene,
            layer: f.layer,
            change: flare_commands::LayerChange::Locked(true),
        }])
        .expect("lock");
    assert_eq!(
        lay(&mut f, &[line((0.0, 0.0), (10.0, 0.0), 1.0)]),
        Err(LayRefusal::Draw(DrawRefusal::Locked))
    );
}

#[test]
fn the_stabilizer_steadies_a_wobble_and_ends_where_the_pen_lifted() {
    let wobble: Vec<InputSample> = (0..=40)
        .map(|i| {
            let y = if i % 2 == 0 { 4.0 } else { -4.0 };
            InputSample::new(f64::from(i) * 5.0, y, 1.0, PointerKind::Pen).expect("sample")
        })
        .collect();
    let steadied = steady(&wobble, 8);
    let swing = |samples: &[InputSample]| {
        samples[10..samples.len() - 10]
            .iter()
            .map(|s| s.y.abs())
            .fold(0.0, f64::max)
    };
    assert!(swing(&steadied) < swing(&wobble) / 2.0);
    let (last, end) = (wobble[wobble.len() - 1], steadied[steadied.len() - 1]);
    assert_eq!((end.x, end.y), (last.x, last.y));
    assert_eq!(steady(&wobble, 0), wobble);
}

#[test]
fn profiles_are_found_by_key_or_name() {
    assert_eq!(
        BuiltinBrushProfile::find("g-pen"),
        Some(BuiltinBrushProfile::GPen)
    );
    assert_eq!(
        BuiltinBrushProfile::find("GPen"),
        Some(BuiltinBrushProfile::GPen)
    );
    assert_eq!(
        BuiltinBrushProfile::find("Krita-style Ink"),
        Some(BuiltinBrushProfile::Ink)
    );
    assert_eq!(
        BuiltinBrushProfile::find("dry brush"),
        Some(BuiltinBrushProfile::DryBrush)
    );
    assert_eq!(BuiltinBrushProfile::find("watercolour"), None);
    assert_eq!(BuiltinBrushProfile::find("--"), None);
    for key in BuiltinBrushProfile::keys() {
        assert!(BuiltinBrushProfile::find(&key).is_some(), "{key}");
    }
}
