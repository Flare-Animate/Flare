//! The Add, Delete and Convert Anchor Point operations.

use super::*;
use flare_document::PathSegment;

fn path_of(f: &Fixture, start: u32) -> Vec<PathSegment> {
    let Content::Shape(shape) = find_span(f.session.document(), f.scene, at(f, start))
        .expect("span")
        .content()
    else {
        panic!("shape");
    };
    shape.path().to_vec()
}

fn anchor_count(f: &Fixture) -> usize {
    path_of(f, 0)
        .iter()
        .filter(|s| !matches!(s, PathSegment::Close))
        .count()
}

fn rect_fixture() -> Fixture {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    f
}

fn circle_fixture() -> Fixture {
    let mut f = fixture();
    let circle = Shape::ellipse(ShapeId::new(), 50.0, 50.0, 20.0, 20.0, RED).expect("circle");
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(circle)),
    );
    f
}

fn run(f: &mut Fixture, commands: Option<Vec<Command>>) {
    f.session.apply(commands.expect("an edit")).expect("apply");
}

#[test]
fn an_anchor_added_on_an_edge_splits_it_and_leaves_the_outline_as_it_was() {
    let mut f = rect_fixture();
    let before = bounds(&f, 0);
    let top = add_anchor(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(5.0, 0.4),
        2.0,
    );
    run(&mut f, top);
    assert_eq!(anchor_count(&f), 5);
    assert_eq!(bounds(&f, 0), before);
    let points = path_of(&f, 0);
    assert!(
        points
            .iter()
            .any(|s| matches!(s, PathSegment::LineTo(p) if p.as_vec() == DVec2::new(5.0, 0.0))),
        "the new point is on the edge: {points:?}"
    );
    // The edge Close draws back to the start can be split too (the left edge).
    let left = add_anchor(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(0.0, 5.0),
        2.0,
    );
    run(&mut f, left);
    assert_eq!(anchor_count(&f), 6);
    assert!(matches!(path_of(&f, 0).last(), Some(PathSegment::Close)));
    assert!(f.session.undo() && f.session.undo());
    assert_eq!(anchor_count(&f), 4, "each was one undo step");
}

#[test]
fn nothing_is_added_away_from_the_outline_at_a_corner_or_on_something_that_is_not_a_shape() {
    let f = rect_fixture();
    let doc = f.session.document();
    let near = |x, y, tol| add_anchor(doc, f.scene, at(&f, 0), DVec2::new(x, y), tol);
    assert!(near(5.0, 5.0, 2.0).is_none(), "inside, away from any edge");
    assert!(
        near(10.0, 0.0, 2.0).is_none(),
        "at a corner there is a point already"
    );
    assert!(near(5.0, 0.0, 2.0).is_some());
    assert!(near(5.0, 1.0, 0.5).is_none(), "outside the tolerance");
    assert!(near(f64::NAN, 0.0, 2.0).is_none(), "not a number");
    assert!(
        add_anchor(
            doc,
            f.scene,
            SpanRef {
                layer: f.layer,
                start: 99
            },
            DVec2::ZERO,
            2.0
        )
        .is_none()
    );
}

#[test]
fn an_anchor_added_on_a_curve_lies_on_it_and_the_two_halves_are_the_same_curve() {
    let mut f = circle_fixture();
    // The point of the circle at 45 degrees.
    let r = 20.0 * std::f64::consts::FRAC_1_SQRT_2;
    let near = DVec2::new(50.0 + r + 0.5, 50.0 + r + 0.5);
    let commands = add_anchor(f.session.document(), f.scene, at(&f, 0), near, 3.0);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 6);
    let path = path_of(&f, 0);
    let PathSegment::CubicTo(_, _, p) = path[2] else {
        panic!("the split curve");
    };
    let on_circle = (p.as_vec() - DVec2::new(50.0, 50.0)).length();
    assert!(
        (on_circle - 20.0).abs() < 0.05,
        "{on_circle} is off the circle"
    );
}

#[test]
fn deleting_a_corner_joins_its_neighbours_and_a_shape_keeps_its_least_points() {
    let mut f = rect_fixture();
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 2);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 3, "a triangle");
    assert!(
        delete_anchor(f.session.document(), f.scene, at(&f, 0), 1).is_none(),
        "a closed shape keeps three points"
    );
    assert!(f.session.undo());
    assert_eq!(anchor_count(&f), 4);
    // The first point goes too, and the outline is what is left.
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 0);
    run(&mut f, commands);
    let b = bounds(&f, 0);
    assert_eq!((b.min, b.max), (Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)));
    assert_eq!(anchor_count(&f), 3);
    // Close is not a point, a missing segment is not either.
    let f = rect_fixture();
    assert!(delete_anchor(f.session.document(), f.scene, at(&f, 0), 4).is_none());
    assert!(delete_anchor(f.session.document(), f.scene, at(&f, 0), 99).is_none());
}

#[test]
fn deleting_a_point_of_an_open_line_shortens_it_down_to_two_points() {
    let mut f = fixture();
    let stroke = flare_document::Stroke::new(2.0, RED).expect("stroke");
    let p = |x, y| flare_core::DocumentPoint::new(x, y).expect("point");
    let path = vec![
        PathSegment::MoveTo(p(0.0, 0.0)),
        PathSegment::LineTo(p(5.0, 5.0)),
        PathSegment::LineTo(p(10.0, 0.0)),
    ];
    let shape = Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
        .expect("shape")
        .with_stroke(stroke);
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape)),
    );
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 2);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 2);
    assert!(delete_anchor(f.session.document(), f.scene, at(&f, 0), 1).is_none());
}

#[test]
fn deleting_a_curve_point_makes_one_curve_where_there_were_two() {
    let mut f = circle_fixture();
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 2);
    run(&mut f, commands);
    let path = path_of(&f, 0);
    assert_eq!(anchor_count(&f), 4);
    assert!(matches!(path[2], PathSegment::CubicTo(..)), "still a curve");
    // The first point of a path that comes back to it by a segment of its own.
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 0);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 3);
    assert!(f.session.undo() && f.session.undo());
    assert_eq!(anchor_count(&f), 5, "the circle again, one undo for each");
}

#[test]
fn a_click_makes_a_curve_point_a_corner_and_there_is_nothing_to_do_to_a_corner() {
    let mut f = circle_fixture();
    let commands = convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, None);
    run(&mut f, commands);
    let path = path_of(&f, 0);
    let (PathSegment::CubicTo(_, c2, p), PathSegment::CubicTo(c1, _, _)) = (path[1], path[2])
    else {
        panic!("curves");
    };
    assert_eq!((c2, c1), (p, p), "both handles are pulled in to the point");
    assert!(
        convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, None).is_none(),
        "already a corner"
    );
    let r = rect_fixture();
    assert!(
        convert_anchor(r.session.document(), r.scene, at(&r, 0), 1, None).is_none(),
        "straight edges have no handles"
    );
}

#[test]
fn a_drag_makes_a_corner_smooth_with_mirrored_handles_and_curves_the_lines_beside_it() {
    let mut f = rect_fixture();
    let to = DVec2::new(10.0 + 3.0, 0.0 - 4.0);
    let commands = convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, Some(to));
    run(&mut f, commands);
    let path = path_of(&f, 0);
    let (PathSegment::CubicTo(_, arriving, p), PathSegment::CubicTo(leaving, _, _)) =
        (path[1], path[2])
    else {
        panic!("both sides are curves now: {path:?}");
    };
    let corner = p.as_vec();
    assert_eq!(corner, DVec2::new(10.0, 0.0));
    assert_eq!(leaving.as_vec(), to, "the handle follows the drag");
    assert_eq!(
        arriving.as_vec(),
        corner - (to - corner),
        "the other is its mirror"
    );
    assert!(
        matches!(path[3], PathSegment::LineTo(_)),
        "farther edges stay straight"
    );
    let none = convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, Some(corner));
    assert!(none.is_none(), "a drag of no length changes nothing");
    assert!(f.session.undo());
    assert!(
        path_of(&f, 0)
            .iter()
            .all(|s| !matches!(s, PathSegment::CubicTo(..)))
    );
}

#[test]
fn converting_needs_a_shape_and_a_point() {
    let f = rect_fixture();
    let doc = f.session.document();
    assert!(
        convert_anchor(doc, f.scene, at(&f, 0), 4, Some(DVec2::ONE)).is_none(),
        "Close"
    );
    assert!(convert_anchor(doc, f.scene, at(&f, 0), 99, Some(DVec2::ONE)).is_none());
    assert!(
        convert_anchor(
            doc,
            f.scene,
            SpanRef {
                layer: f.layer,
                start: 7
            },
            1,
            Some(DVec2::ONE)
        )
        .is_none()
    );
}
