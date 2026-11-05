#![allow(clippy::expect_used)]

use super::*;
use emath::Pos2;
use flare_core::DocumentPoint;
use flare_document::PathSegment;

fn p(x: f32, y: f32) -> Pos2 {
    Pos2::new(x, y)
}

fn click(path: &mut PenPath, at: Pos2) -> Press {
    let outcome = path.press(at);
    path.release();
    outcome
}

fn xy(point: &DocumentPoint) -> (f64, f64) {
    (point.as_vec().x, point.as_vec().y)
}

#[test]
fn clicks_make_a_polyline() {
    let mut path = PenPath::default();
    for at in [p(0.0, 0.0), p(50.0, 0.0), p(50.0, 40.0)] {
        assert_eq!(click(&mut path, at), Press::Added);
    }
    let segments = path.segments(false).expect("segments");
    assert_eq!(segments.len(), 3);
    assert!(matches!(segments[0], PathSegment::MoveTo(_)));
    assert!(
        segments[1..]
            .iter()
            .all(|s| matches!(s, PathSegment::LineTo(_)))
    );
}

#[test]
fn dragging_a_point_sets_its_handle_and_the_incoming_side_mirrors_it() {
    let mut path = PenPath::default();
    assert_eq!(path.press(p(100.0, 100.0)), Press::Added);
    path.drag(p(140.0, 100.0));
    path.release();
    assert_eq!(path.points()[0].out, Some(p(140.0, 100.0)));
    // Second point is dragged too, so its incoming handle is the mirror of its outgoing one.
    path.press(p(300.0, 100.0));
    path.drag(p(340.0, 120.0));
    path.release();
    let segments = path.segments(false).expect("segments");
    let PathSegment::CubicTo(c1, c2, to) = &segments[1] else {
        panic!("a curve expected")
    };
    assert_eq!(xy(c1), (140.0, 100.0), "the first point's dragged handle");
    assert_eq!(
        xy(c2),
        (260.0, 80.0),
        "mirror of (340, 120) through (300, 100)"
    );
    assert_eq!(xy(to), (300.0, 100.0));
}

#[test]
fn a_curve_needs_only_one_dragged_end() {
    let mut path = PenPath::default();
    path.press(p(0.0, 0.0));
    path.drag(p(0.0, 30.0));
    path.release();
    click(&mut path, p(100.0, 0.0));
    let segments = path.segments(false).expect("segments");
    let PathSegment::CubicTo(c1, c2, _) = &segments[1] else {
        panic!("a curve expected")
    };
    assert_eq!(xy(c1), (0.0, 30.0));
    assert_eq!(
        xy(c2),
        (100.0, 0.0),
        "no handle on the corner end: it sits on its anchor"
    );
}

#[test]
fn tiny_drags_stay_corner_points() {
    let mut path = PenPath::default();
    path.press(p(10.0, 10.0));
    path.drag(p(10.4, 10.3));
    path.release();
    assert_eq!(path.points()[0].out, None);
    // Dragging without a press does nothing.
    path.drag(p(99.0, 99.0));
    assert_eq!(path.points()[0].out, None);
}

#[test]
fn clicking_the_first_point_closes_only_with_three_or_more_points() {
    let mut path = PenPath::default();
    click(&mut path, p(0.0, 0.0));
    click(&mut path, p(60.0, 0.0));
    assert_eq!(
        path.press(p(2.0, 2.0)),
        Press::Added,
        "two points cannot close; this is a third point"
    );
    path.release();
    path.clear();
    for at in [p(0.0, 0.0), p(60.0, 0.0), p(60.0, 60.0)] {
        click(&mut path, at);
    }
    assert_eq!(path.press(p(3.0, 3.0)), Press::Close);
    assert_eq!(path.points().len(), 3, "closing adds no point");
    assert_eq!(
        path.press(p(30.0, 30.0)),
        Press::Added,
        "far from the start it is just another point"
    );
}

#[test]
fn closed_paths_end_with_close_and_curved_closing_edges_are_explicit() {
    let mut path = PenPath::default();
    for at in [p(0.0, 0.0), p(60.0, 0.0), p(60.0, 60.0)] {
        click(&mut path, at);
    }
    let straight = path.segments(true).expect("closed");
    assert!(matches!(straight.last(), Some(PathSegment::Close)));
    assert_eq!(
        straight.len(),
        4,
        "a straight closing edge is implied by Close"
    );

    path.clear();
    path.press(p(0.0, 0.0));
    path.drag(p(0.0, -30.0));
    path.release();
    click(&mut path, p(60.0, 0.0));
    click(&mut path, p(60.0, 60.0));
    let curved = path.segments(true).expect("closed");
    assert!(
        matches!(curved[curved.len() - 2], PathSegment::CubicTo(..)),
        "the closing edge is a curve"
    );
    assert!(matches!(curved.last(), Some(PathSegment::Close)));
}

#[test]
fn too_few_points_give_no_path() {
    let mut path = PenPath::default();
    assert!(path.segments(false).is_none());
    click(&mut path, p(0.0, 0.0));
    assert!(path.segments(false).is_none(), "one point is not a path");
    click(&mut path, p(5.0, 0.0));
    assert!(path.segments(false).is_some());
    assert!(path.segments(true).is_none(), "two points cannot be closed");
}

#[test]
fn backspace_removes_points_and_a_double_click_duplicate_is_trimmed() {
    let mut path = PenPath::default();
    for at in [p(0.0, 0.0), p(40.0, 0.0), p(40.0, 40.0), p(40.5, 40.2)] {
        click(&mut path, at);
    }
    path.trim_duplicates();
    assert_eq!(
        path.points().len(),
        3,
        "the near-duplicate from the second click went"
    );
    path.remove_last();
    assert_eq!(path.points().len(), 2);
    path.remove_last();
    path.remove_last();
    path.remove_last(); // removing from empty is harmless
    assert!(!path.is_active());
}

#[test]
fn bad_input_is_ignored_and_the_path_is_bounded() {
    let mut path = PenPath::default();
    assert_eq!(path.press(p(f32::NAN, 0.0)), Press::Ignored);
    assert_eq!(path.press(p(0.0, f32::INFINITY)), Press::Ignored);
    path.press(p(1.0, 1.0));
    path.drag(p(f32::NAN, 5.0));
    assert_eq!(
        path.points()[0].out,
        None,
        "a non-finite handle is not stored"
    );
    path.clear();
    for i in 0..MAX_POINTS {
        assert_eq!(click(&mut path, p(i as f32 * 10.0, 0.0)), Press::Added);
    }
    assert_eq!(
        path.press(p(-500.0, 500.0)),
        Press::Ignored,
        "the point limit holds"
    );
    assert_eq!(path.points().len(), MAX_POINTS);
}

mod shapes {
    use super::*;
    use flare_core::{DocumentPoint, Rgba8, ShapeId};
    use flare_document::{Cap, Join, PathSegment, ShapeError};

    fn triangle() -> Vec<PathSegment> {
        let p = |x, y| DocumentPoint::new(x, y).expect("point");
        vec![
            PathSegment::MoveTo(p(0.0, 0.0)),
            PathSegment::LineTo(p(10.0, 0.0)),
            PathSegment::LineTo(p(0.0, 10.0)),
            PathSegment::Close,
        ]
    }

    fn style(outline: bool, width: f64) -> PenStyle {
        PenStyle {
            fill: Rgba8::opaque(10, 20, 30),
            outline,
            stroke_width: width,
            stroke_color: Rgba8::opaque(200, 0, 0),
            cap: Cap::Square,
            join: Join::Miter,
            style: flare_document::StrokeStyle::Dotted,
        }
    }

    #[test]
    fn a_closed_path_is_filled_and_outlined_only_when_asked() {
        let plain =
            path_shape(ShapeId::new(), triangle(), true, &style(false, 2.0)).expect("shape");
        assert_eq!(plain.fill(), Rgba8::opaque(10, 20, 30));
        assert!(plain.stroke().is_none());
        let outlined =
            path_shape(ShapeId::new(), triangle(), true, &style(true, 3.0)).expect("shape");
        let stroke = outlined.stroke().expect("an outline");
        assert_eq!(
            (stroke.width(), stroke.cap(), stroke.join()),
            (3.0, Cap::Square, Join::Miter)
        );
    }

    #[test]
    fn an_open_path_has_no_fill_and_is_always_drawn_with_the_stroke() {
        let line = path_shape(
            ShapeId::new(),
            triangle()[..3].to_vec(),
            false,
            &style(false, 2.0),
        )
        .expect("shape");
        assert_eq!(line.fill(), Rgba8::TRANSPARENT);
        assert_eq!(
            line.stroke().map(|s| s.color()),
            Some(Rgba8::opaque(200, 0, 0))
        );
    }

    #[test]
    fn what_cannot_be_a_shape_is_an_error_not_a_panic() {
        assert_eq!(
            path_shape(ShapeId::new(), Vec::new(), true, &style(false, 2.0)).err(),
            Some(ShapeError::Empty)
        );
        // An outline with no width cannot be drawn.
        assert_eq!(
            path_shape(ShapeId::new(), triangle(), true, &style(true, 0.0)).err(),
            Some(ShapeError::InvalidStrokeWidth)
        );
        // Without an outline the width does not matter.
        assert!(path_shape(ShapeId::new(), triangle(), true, &style(false, 0.0)).is_ok());
    }
}
