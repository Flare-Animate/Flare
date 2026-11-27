#![allow(clippy::expect_used)]

use crate::eraser::vector::{Take, erase_with, rectangle};
use crate::test_support::*;
use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{Content, FrameSpan, PathSegment, Shape, Stroke};
use glamx::DVec2;
use std::num::NonZeroU32;

fn point(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("point")
}

/// A line 2 wide through `points`, closed back to its start when `closed`.
fn line(points: &[(f64, f64)], closed: bool) -> Content {
    let mut path = vec![PathSegment::MoveTo(point(points[0].0, points[0].1))];
    path.extend(
        points[1..]
            .iter()
            .map(|p| PathSegment::LineTo(point(p.0, p.1))),
    );
    if closed {
        path.push(PathSegment::Close);
    }
    Content::Shape(
        Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
            .expect("line")
            .with_stroke(Stroke::new(2.0, RED).expect("stroke")),
    )
}

/// A filled box with no line, as a Brush stroke is a fill.
fn fill(x: f64, y: f64, w: f64, h: f64) -> Content {
    Content::Shape(Shape::rect(ShapeId::new(), x, y, w, h, RED).expect("fill"))
}

fn drawn(f: &mut Fixture, elements: Vec<Content>) {
    let first = elements[0].clone();
    let span = FrameSpan::new(0, NonZeroU32::MIN, first)
        .with_elements(elements)
        .expect("elements")
        .expect("not empty");
    add(f, span);
}

/// Every element at frame 0: its points (curves are not used here), and whether it has a line.
fn shapes(f: &Fixture) -> Vec<(Vec<DVec2>, bool)> {
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    let Some(span) = layer.span_at(0) else {
        return Vec::new();
    };
    span.elements()
        .filter_map(|c| match c {
            Content::Shape(shape) => Some((
                shape
                    .path()
                    .iter()
                    .filter_map(|s| match s {
                        PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec()),
                        _ => None,
                    })
                    .collect(),
                shape.stroke().is_some(),
            )),
            _ => None,
        })
        .collect()
}

/// The subpaths (runs from one MoveTo to the next) of the element at frame 0 that `pick` finds.
fn runs(f: &Fixture, pick: impl Fn(&Shape) -> bool) -> usize {
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    layer
        .span_at(0)
        .and_then(|span| {
            span.elements().find_map(|c| match c {
                Content::Shape(shape) if pick(shape) => Some(
                    shape
                        .path()
                        .iter()
                        .filter(|s| matches!(s, PathSegment::MoveTo(_)))
                        .count(),
                ),
                _ => None,
            })
        })
        .unwrap_or(0)
}

/// Erases with the Segment type under a small box round `(x, y)`; gives whether it took anything.
fn rub(f: &mut Fixture, x: f64, y: f64) -> bool {
    let tip = rectangle(DVec2::new(x - 2.0, y - 2.0), DVec2::new(x + 2.0, y + 2.0));
    let commands = erase_with(
        f.session.document(),
        f.scene,
        0,
        &[tip],
        Take::Segments,
        None,
    );
    let any = !commands.is_empty();
    if any {
        f.session.apply(commands).expect("erase");
    }
    any
}

fn span_x(points: &[DVec2]) -> (f64, f64) {
    points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.x), hi.max(p.x))
    })
}

#[test]
fn a_crossed_line_loses_only_the_stretch_out_to_the_crossing() {
    let mut f = fixture();
    // A line across, and one down through it at x = 50.
    drawn(
        &mut f,
        vec![
            line(&[(0.0, 0.0), (100.0, 0.0)], false),
            line(&[(50.0, -50.0), (50.0, 50.0)], false),
        ],
    );
    assert!(rub(&mut f, 80.0, 0.0));
    let after = shapes(&f);
    assert_eq!(after.len(), 2, "both lines are still there");
    let (lo, hi) = span_x(&after[0].0);
    assert_eq!(lo, 0.0, "the line keeps its start");
    assert!(
        (hi - 51.0).abs() < 1e-6,
        "and ends at the far side of the line crossing it, not at {hi}"
    );
    assert_eq!(
        span_x(&after[1].0),
        (50.0, 50.0),
        "the crossing line is whole"
    );
    assert_eq!(
        f.session.undo_labels().count(),
        2,
        "the lines, then one step"
    );
}

#[test]
fn between_two_crossings_only_the_middle_goes_and_what_meets_stays_one_line() {
    let mut f = fixture();
    drawn(
        &mut f,
        vec![
            line(&[(0.0, 0.0), (100.0, 0.0)], false),
            line(&[(30.0, -20.0), (30.0, 20.0)], false),
            line(&[(70.0, -20.0), (70.0, 20.0)], false),
        ],
    );
    assert!(rub(&mut f, 50.0, 0.0));
    let across = &shapes(&f)[0].0;
    assert!(
        across
            .iter()
            .all(|p| p.x <= 31.0 + 1e-6 || p.x >= 69.0 - 1e-6)
    );
    assert_eq!(span_x(across), (0.0, 100.0), "both ends are kept");
    assert_eq!(
        runs(&f, |s| s.path().len() > 2),
        2,
        "two pieces, not one for each stretch between the crossings' edges"
    );
}

#[test]
fn a_line_nothing_crosses_goes_whole_and_one_over_an_area_is_not_crossed_by_it() {
    let mut f = fixture();
    // An area filled under the line, as the Paint Bucket pours one: far wider than a line.
    drawn(
        &mut f,
        vec![
            fill(-100.0, -100.0, 300.0, 200.0),
            line(&[(0.0, 0.0), (100.0, 0.0)], false),
        ],
    );
    assert!(rub(&mut f, 80.0, 0.0));
    let after = shapes(&f);
    assert_eq!(
        after.len(),
        1,
        "the line went whole; the area is left for the Eraser"
    );
    assert!(!after[0].1, "what is left is the area");
    assert!(
        !rub(&mut f, 80.0, 0.0),
        "the Segment type does not erase an area"
    );
}

#[test]
fn a_brush_stroke_crossed_by_another_is_sliced_at_its_edges() {
    let mut f = fixture();
    // Two strokes as the Brush makes them: thin fills, one across the other at x = 48 to 52.
    drawn(
        &mut f,
        vec![fill(0.0, -2.0, 100.0, 4.0), fill(48.0, -50.0, 4.0, 100.0)],
    );
    assert!(rub(&mut f, 80.0, 0.0));
    let after = shapes(&f);
    assert_eq!(after.len(), 2);
    let (lo, hi) = span_x(&after[0].0);
    assert!(
        (lo - 0.0).abs() < 1e-6 && (hi - 52.0).abs() < 1e-3,
        "{lo} to {hi}"
    );
    assert_eq!(
        span_x(&after[1].0),
        (48.0, 52.0),
        "the crossing stroke is whole"
    );
}

#[test]
fn a_loop_crossed_once_loses_the_long_way_round_and_keeps_the_corner_in_one_piece() {
    let mut f = fixture();
    drawn(
        &mut f,
        vec![
            line(
                &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                true,
            ),
            // Across the top-left corner: crosses the top side and the left side.
            line(&[(-20.0, 30.0), (30.0, -20.0)], false),
        ],
    );
    // The bottom side is on the stretch that runs from the top crossing round to the left one,
    // past the loop's start, so all of that goes.
    assert!(rub(&mut f, 50.0, 100.0));
    let corner = shapes(&f)[0].0.clone();
    assert!(
        corner.iter().all(|p| p.x < 12.0 && p.y < 12.0),
        "only the corner the line cuts off is left: {corner:?}"
    );
    assert!(corner.contains(&DVec2::ZERO), "the corner itself");
    assert_eq!(
        runs(&f, |s| s.path().len() > 2),
        1,
        "in one piece across the loop's start"
    );
}

#[test]
fn an_outlined_shape_keeps_its_fill_and_loses_part_of_its_outline() {
    let mut f = fixture();
    let boxed = Shape::rect(ShapeId::new(), 0.0, 0.0, 40.0, 40.0, RED)
        .expect("box")
        .with_stroke(Stroke::new(2.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    drawn(
        &mut f,
        vec![
            Content::Shape(boxed),
            line(&[(20.0, -10.0), (20.0, 50.0)], false),
        ],
    );
    assert!(rub(&mut f, 30.0, 0.0));
    let after = shapes(&f);
    assert_eq!(
        after.len(),
        3,
        "the fill, what is left of the outline, and the line"
    );
    assert!(!after[0].1, "the fill, on its own and whole");
    assert_eq!(span_x(&after[0].0), (0.0, 40.0));
    assert!(after[1].1 && !after[1].0.iter().any(|p| p.y == 0.0 && p.x > 22.0));
}

#[test]
#[ignore = "a measurement, run by hand in release"]
fn measure_segment_erasing() {
    let mut f = fixture();
    // A 600-unit Brush-like stroke with 120 points, crossed by 60 others of 40 points each.
    let ribbon = |points: Vec<(f64, f64)>| {
        let mut path = vec![PathSegment::MoveTo(point(points[0].0, points[0].1))];
        path.extend(
            points[1..]
                .iter()
                .map(|p| PathSegment::LineTo(point(p.0, p.1))),
        );
        path.push(PathSegment::Close);
        Content::Shape(Shape::new(ShapeId::new(), path, RED).expect("ribbon"))
    };
    let across: Vec<(f64, f64)> = (0..60)
        .map(|i| (i as f64 * 10.0, -3.0))
        .chain((0..60).rev().map(|i| (i as f64 * 10.0, 3.0)))
        .collect();
    let mut elements = vec![ribbon(across)];
    for k in 0..60 {
        let x = 5.0 + k as f64 * 10.0;
        let down: Vec<(f64, f64)> = (0..20)
            .map(|i| (x - 2.0, -50.0 + i as f64 * 5.0))
            .chain((0..20).rev().map(|i| (x + 2.0, -50.0 + i as f64 * 5.0)))
            .collect();
        elements.push(ribbon(down));
    }
    drawn(&mut f, elements);
    let tip = crate::eraser::eraser_outline(
        &[
            flare_input::InputSample::mouse(300.0, -10.0).expect("a"),
            flare_input::InputSample::mouse(302.0, 10.0).expect("b"),
        ],
        20.0,
        false,
    );
    let start = std::time::Instant::now();
    let rounds = 60;
    for _ in 0..rounds {
        let commands = erase_with(
            f.session.document(),
            f.scene,
            0,
            std::slice::from_ref(&tip),
            Take::Segments,
            None,
        );
        assert!(!commands.is_empty());
    }
    eprintln!("segment erase: {:?} a frame", start.elapsed() / rounds);
}
