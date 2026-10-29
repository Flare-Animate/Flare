#![allow(clippy::expect_used)]

use super::*;

fn p(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

fn shape(path: Vec<PathSegment>) -> Result<Shape, ShapeError> {
    Shape::new(ShapeId::new(), path, Rgba8::WHITE)
}

#[test]
fn well_formed_paths_are_accepted() {
    assert!(Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 5.0, Rgba8::WHITE).is_some());
    assert!(Shape::rect(ShapeId::new(), f64::NAN, 0.0, 1.0, 1.0, Rgba8::WHITE).is_none());
}

#[test]
fn ellipses_and_lines_are_well_formed() {
    let ellipse =
        Shape::ellipse(ShapeId::new(), 10.0, 20.0, 5.0, 3.0, Rgba8::WHITE).expect("ellipse");
    assert_eq!(ellipse.path().len(), 6);
    assert!(matches!(ellipse.path()[5], PathSegment::Close));
    // The four anchor points sit on the axes at the given radii.
    let anchors: Vec<(f64, f64)> = ellipse
        .path()
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) => Some((p.as_vec().x, p.as_vec().y)),
            PathSegment::CubicTo(_, _, p) => Some((p.as_vec().x, p.as_vec().y)),
            _ => None,
        })
        .collect();
    assert_eq!(
        anchors,
        vec![
            (15.0, 20.0),
            (10.0, 23.0),
            (5.0, 20.0),
            (10.0, 17.0),
            (15.0, 20.0)
        ]
    );
    assert!(Shape::ellipse(ShapeId::new(), f64::NAN, 0.0, 1.0, 1.0, Rgba8::WHITE).is_none());

    let stroke = Stroke::new(2.0, Rgba8::WHITE).expect("stroke");
    let line = Shape::line(ShapeId::new(), (0.0, 0.0), (4.0, 3.0), stroke).expect("line");
    assert_eq!(line.path().len(), 2);
    assert_eq!(line.fill(), Rgba8::TRANSPARENT);
    assert_eq!(line.stroke(), Some(stroke));
    assert!(Shape::line(ShapeId::new(), (0.0, 0.0), (f64::INFINITY, 0.0), stroke).is_none());
}

#[test]
fn strokes_validate_their_width() {
    assert!(Stroke::new(2.5, Rgba8::WHITE).is_ok());
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, MAX_STROKE_WIDTH + 1.0] {
        assert_eq!(
            Stroke::new(bad, Rgba8::WHITE),
            Err(ShapeError::InvalidStrokeWidth)
        );
    }
    let stroke = Stroke::new(3.0, Rgba8::opaque(1, 2, 3)).expect("stroke");
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 1.0, 1.0, Rgba8::WHITE)
        .expect("rect")
        .with_stroke(stroke);
    assert_eq!(shape.stroke(), Some(stroke));
}

#[test]
fn malformed_paths_are_rejected() {
    assert_eq!(shape(vec![]), Err(ShapeError::Empty));
    assert_eq!(
        shape(vec![PathSegment::Close]),
        Err(ShapeError::MustStartWithMove)
    );
    let line_after_close = vec![
        PathSegment::MoveTo(p(0.0, 0.0)),
        PathSegment::Close,
        PathSegment::LineTo(p(1.0, 1.0)),
    ];
    assert_eq!(shape(line_after_close), Err(ShapeError::NoCurrentPoint));
    let too_long = vec![PathSegment::MoveTo(p(0.0, 0.0)); MAX_SEGMENTS + 1];
    assert_eq!(shape(too_long), Err(ShapeError::TooManySegments));
}

#[test]
fn gradients_need_two_different_points_and_report_visibility() {
    assert_eq!(
        Gradient::linear(
            p(1.0, 1.0),
            p(1.0, 1.0),
            Rgba8::WHITE,
            Rgba8::opaque(0, 0, 0)
        ),
        Err(GradientError::SamePoint)
    );
    let clear = Gradient::linear(
        p(0.0, 0.0),
        p(1.0, 0.0),
        Rgba8::TRANSPARENT,
        Rgba8::TRANSPARENT,
    )
    .expect("gradient");
    assert!(clear.is_invisible());
    let faded = Gradient::linear(p(0.0, 0.0), p(1.0, 0.0), Rgba8::TRANSPARENT, Rgba8::WHITE)
        .expect("gradient");
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::TRANSPARENT).expect("rect");
    assert!(shape.fill_is_invisible());
    let shape = shape.with_gradient(faded);
    assert!(
        !shape.fill_is_invisible(),
        "a visible gradient paints even over a clear fill"
    );
    assert_eq!(shape.gradient(), Some(faded));
    assert!(shape.with_gradient(clear).fill_is_invisible());
}

fn bounds_of(shape: &Shape) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in shape.path() {
        let points: Vec<&DocumentPoint> = match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => vec![p],
            PathSegment::CubicTo(a, c, p) => vec![a, c, p],
            PathSegment::Close => vec![],
        };
        for p in points {
            let v = p.as_vec();
            b = (b.0.min(v.x), b.1.min(v.y), b.2.max(v.x), b.3.max(v.y));
        }
    }
    b
}

#[test]
fn rounded_rectangles_keep_their_bounds_clamp_the_radius_and_square_off_at_zero() {
    let id = ShapeId::new();
    let r = Shape::rounded_rect(id, 10.0, 20.0, 100.0, 50.0, 8.0, Rgba8::WHITE).expect("rounded");
    assert_eq!(bounds_of(&r), (10.0, 20.0, 110.0, 70.0));
    assert_eq!(
        r.path()
            .iter()
            .filter(|s| matches!(s, PathSegment::CubicTo(..)))
            .count(),
        4
    );
    // A radius bigger than half the short side is a pill, not a bulge outside the box.
    let pill = Shape::rounded_rect(id, 0.0, 0.0, 100.0, 20.0, 999.0, Rgba8::WHITE).expect("pill");
    assert_eq!(bounds_of(&pill), (0.0, 0.0, 100.0, 20.0));
    let square = Shape::rounded_rect(id, 0.0, 0.0, 10.0, 10.0, 0.0, Rgba8::WHITE).expect("square");
    assert_eq!(square.path().len(), 5, "zero radius is the plain rectangle");
    assert!(Shape::rounded_rect(id, f64::NAN, 0.0, 10.0, 10.0, 2.0, Rgba8::WHITE).is_none());
}

#[test]
fn polygons_and_stars_have_the_right_corners_on_the_right_circle() {
    let id = ShapeId::new();
    let tri = Shape::polygon(id, (50.0, 50.0), 10.0, 3, 0.0, None, Rgba8::WHITE).expect("triangle");
    let corners: Vec<(f64, f64)> = tri
        .path()
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some((p.as_vec().x, p.as_vec().y)),
            _ => None,
        })
        .collect();
    assert_eq!(corners.len(), 3);
    assert!(
        (corners[0].0 - 50.0).abs() < 1e-9 && (corners[0].1 - 40.0).abs() < 1e-9,
        "first corner straight up"
    );
    for (x, y) in &corners {
        let d = ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt();
        assert!((d - 10.0).abs() < 1e-9, "on the circle: {d}");
    }
    let star = Shape::polygon(id, (0.0, 0.0), 10.0, 5, 0.0, Some(0.5), Rgba8::WHITE).expect("star");
    let radii: Vec<f64> = star
        .path()
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec().length()),
            _ => None,
        })
        .collect();
    assert_eq!(radii.len(), 10, "five points, five notches");
    for (i, r) in radii.iter().enumerate() {
        let want = if i % 2 == 0 { 10.0 } else { 5.0 };
        assert!((r - want).abs() < 1e-9, "{i}: {r}");
    }
    for (sides, radius) in [(2, 5.0), (65, 5.0), (5, 0.0), (5, -1.0), (5, f64::NAN)] {
        assert!(Shape::polygon(id, (0.0, 0.0), radius, sides, 0.0, None, Rgba8::WHITE).is_none());
    }
}

#[test]
fn strokes_are_round_unless_told_otherwise_and_keep_their_style() {
    let stroke = Stroke::new(3.0, Rgba8::WHITE).expect("stroke");
    assert_eq!((stroke.cap(), stroke.join()), (Cap::Round, Join::Round));
    let square = stroke.with_cap(Cap::Square).with_join(Join::Miter);
    assert_eq!((square.cap(), square.join()), (Cap::Square, Join::Miter));
    assert_eq!(square.width(), 3.0, "the width is unchanged");
    let shape = Shape::line(ShapeId::new(), (0.0, 0.0), (5.0, 0.0), square).expect("line");
    let copy = Shape::new(ShapeId::new(), shape.path().to_vec(), Rgba8::TRANSPARENT)
        .expect("shape")
        .painted_like(&shape);
    assert_eq!(
        copy.stroke(),
        Some(square),
        "copying the paint copies the style"
    );
}
