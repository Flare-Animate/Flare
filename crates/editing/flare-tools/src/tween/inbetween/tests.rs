#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::*;
use flare_core::{DocumentPoint, ShapeId};
use flare_document::{Content, PathSegment, Shape};
use flare_tween::{Easing, HandleKind, Interpolation, Params};

fn stroke_span(start: u32, duration: u32, x: f64) -> FrameSpan {
    let p = |x: f64, y: f64| DocumentPoint::new(x, y).expect("finite");
    let path = vec![
        PathSegment::MoveTo(p(x, 0.0)),
        PathSegment::LineTo(p(x, 10.0)),
    ];
    let shape = Shape::new(ShapeId::new(), path, RED).expect("shape");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

fn first_x(span: &FrameSpan) -> f64 {
    let Content::Shape(s) = span.content() else {
        panic!("shape")
    };
    let PathSegment::MoveTo(p) = s.path()[0] else {
        panic!("move")
    };
    p.as_vec().x
}

fn doc_spans(f: &Fixture) -> Vec<FrameSpan> {
    f.session.document().scenes()[0].timeline().layers()[0]
        .spans()
        .to_vec()
}

#[test]
fn strokes_are_blended_along_the_curve_in_one_undo_step() {
    let mut f = fixture();
    add(&mut f, stroke_span(0, 4, 0.0));
    add(&mut f, stroke_span(4, 2, 8.0));
    let commands = create_inbetweens(f.session.document(), f.scene, at(&f, 0), 3).expect("ok");
    f.session.apply(commands).expect("apply");
    let spans = doc_spans(&f);
    assert_eq!(spans.len(), 5);
    assert_eq!(
        spans
            .iter()
            .map(|s| (s.start(), s.duration().get()))
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 1), (2, 1), (3, 1), (4, 2)]
    );
    // Even speed: a quarter, a half and three quarters of the way to x = 8.
    assert!((first_x(&spans[1]) - 2.0).abs() < 1e-9);
    assert!((first_x(&spans[2]) - 4.0).abs() < 1e-9);
    assert!((first_x(&spans[3]) - 6.0).abs() < 1e-9);
    assert!(f.session.undo());
    assert_eq!(doc_spans(&f).len(), 2);
    assert!(f.session.redo());
    assert_eq!(doc_spans(&f).len(), 5);
}

#[test]
fn the_keyframes_interpolation_bends_the_inbetweens() {
    let mut f = fixture();
    add(&mut f, stroke_span(0, 2, 0.0));
    add(&mut f, stroke_span(2, 2, 8.0));
    let quad = SpanInterp::new(
        Interpolation::Quad,
        Easing::In,
        HandleKind::Auto,
        [0.0; 4],
        Params::default(),
    )
    .expect("interp");
    let set = set_interpolation(f.session.document(), f.scene, at(&f, 0), Some(quad)).expect("set");
    f.session.apply(set).expect("apply");
    assert!(
        set_interpolation(f.session.document(), f.scene, at(&f, 0), Some(quad)).is_none(),
        "already set"
    );
    let commands = create_inbetweens(f.session.document(), f.scene, at(&f, 0), 1).expect("ok");
    f.session.apply(commands).expect("apply");
    // Halfway in time, quad-in is a quarter of the way: 2, not the linear 4.
    assert!((first_x(&doc_spans(&f)[1]) - 2.0).abs() < 1e-9);
}

#[test]
fn bad_requests_are_refused_and_change_nothing() {
    let mut f = fixture();
    add(&mut f, stroke_span(0, 2, 0.0));
    let doc = f.session.document().clone();
    let r = |count| create_inbetweens(&doc, f.scene, at(&f, 0), count);
    assert_eq!(r(1), Err(InbetweenError::NoNext(0)));
    add(&mut f, stroke_span(2, 2, 8.0));
    let doc = f.session.document().clone();
    let r = |s: u32, count| create_inbetweens(&doc, f.scene, at(&f, s), count);
    assert_eq!(r(0, 0), Err(InbetweenError::Count(0)));
    assert_eq!(
        r(0, MAX_INBETWEENS + 1),
        Err(InbetweenError::Count(MAX_INBETWEENS + 1))
    );
    assert_eq!(r(0, 2), Err(InbetweenError::TooShort { have: 2, want: 2 }));
    assert_eq!(r(1, 1), Err(InbetweenError::NoSuchKeyframe(1)));
    assert!(set_interpolation(&doc, f.scene, at(&f, 1), Some(SpanInterp::default())).is_none());
    assert_eq!(f.session.document(), &doc);
}

#[test]
fn content_that_cannot_blend_is_refused() {
    let mut f = fixture();
    add(&mut f, stroke_span(0, 4, 0.0));
    add(
        &mut f,
        FrameSpan::new(4, NonZeroU32::new(2).expect("nonzero"), Content::Empty),
    );
    assert_eq!(
        create_inbetweens(f.session.document(), f.scene, at(&f, 0), 1),
        Err(InbetweenError::Incompatible)
    );
}

#[test]
fn a_closed_shape_morphs_through_the_contour_path() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    add(&mut f, rect_span(4, 2, 20.0, 0.0, 10.0, 10.0));
    let commands = create_inbetweens(f.session.document(), f.scene, at(&f, 0), 1).expect("ok");
    f.session.apply(commands).expect("apply");
    assert_eq!(doc_spans(&f).len(), 3);
}
