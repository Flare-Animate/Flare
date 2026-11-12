//! The Paint Bucket fills the area lines close off, whatever drew them.

use super::*;
use flare_core::DocumentPoint;
use flare_document::{PathSegment, Stroke as ShapeStroke};

const BLUE: Color32 = Color32::from_rgb(20, 40, 200);

/// A workspace whose stage shows at 100%, so a screen point is a stage unit.
fn at_full_size() -> Workspace {
    let mut ws = workspace();
    ws.set_stage_size(550, 400);
    ws.stage_panel = Rect::from_min_size(Pos2::ZERO, Vec2::new(550.0, 400.0));
    ws
}

fn square(path: &mut Vec<PathSegment>, from: f64, to: f64, clockwise: bool) {
    let p = |x, y| DocumentPoint::new(x, y).expect("finite");
    let corners = if clockwise {
        [(from, from), (to, from), (to, to), (from, to)]
    } else {
        [(from, from), (from, to), (to, to), (to, from)]
    };
    path.push(PathSegment::MoveTo(p(corners[0].0, corners[0].1)));
    for (x, y) in &corners[1..] {
        path.push(PathSegment::LineTo(p(*x, *y)));
    }
    path.push(PathSegment::Close);
}

/// A ring as a brush leaves one: a filled band from 100 to 200, its hole from 110 to 190.
fn brush_ring() -> Shape {
    let mut path = Vec::new();
    square(&mut path, 100.0, 200.0, true);
    square(&mut path, 110.0, 190.0, false);
    Shape::new(ShapeId::new(), path, Rgba8::opaque(220, 40, 40)).expect("ring")
}

fn line(from: (f64, f64), to: (f64, f64)) -> Shape {
    let stroke = ShapeStroke::new(3.0, Rgba8::opaque(0, 0, 0)).expect("stroke");
    Shape::line(ShapeId::new(), from, to, stroke).expect("line")
}

fn bounds(content: &Content) -> Rect {
    match content {
        Content::Shape(shape) => ops::shape_bounds(shape).expect("bounds"),
        other => panic!("a shape expected, got {other:?}"),
    }
}

fn fill_of(content: &Content) -> Rgba8 {
    match content {
        Content::Shape(shape) => shape.fill(),
        other => panic!("a shape expected, got {other:?}"),
    }
}

#[test]
fn the_bucket_fills_the_hole_of_a_brush_ring_under_the_ring_in_one_step() {
    let mut ws = at_full_size();
    ws.draw_shape(brush_ring(), "ring");
    let steps = ws.active().undo_labels().count();
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    let all = drawings(&ws);
    assert_eq!(all.len(), 2, "a new fill and the ring");
    assert_eq!(
        fill_of(&all[0]),
        Rgba8::opaque(20, 40, 200),
        "the fill is under"
    );
    let b = bounds(&all[0]);
    assert!(
        b.min.x > 104.0 && b.min.x <= 110.0 && b.max.x >= 190.0 && b.max.x < 196.0,
        "it fills the hole and tucks under the ring, no further: {b:?}"
    );
    assert_eq!(ws.active().undo_labels().count(), steps + 1);
    ws.undo();
    assert_eq!(drawings(&ws).len(), 1);
}

#[test]
fn the_bucket_fills_a_square_closed_by_four_separate_lines() {
    let mut ws = at_full_size();
    for (from, to) in [
        ((100.0, 100.0), (200.0, 100.0)),
        ((200.0, 100.0), (200.0, 200.0)),
        ((200.0, 200.0), (100.0, 200.0)),
        ((100.0, 200.0), (100.0, 100.0)),
    ] {
        ws.draw_shape(line(from, to), "line");
    }
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    let all = drawings(&ws);
    assert_eq!(all.len(), 5);
    let b = bounds(&all[0]);
    assert!(b.min.x < 103.0 && b.max.y > 197.0, "{b:?}");
}

#[test]
fn lines_that_leave_the_area_open_fill_nothing() {
    let mut ws = at_full_size();
    for (from, to) in [
        ((100.0, 100.0), (200.0, 100.0)),
        ((200.0, 100.0), (200.0, 200.0)),
        ((200.0, 200.0), (100.0, 200.0)),
    ] {
        ws.draw_shape(line(from, to), "line");
    }
    let before = ws.document().clone();
    ws.set_tool(Tool::Bucket);
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    assert_eq!(ws.document(), &before, "the fourth side is missing");
}

#[test]
fn an_area_over_a_filled_shape_goes_between_it_and_the_lines() {
    let mut ws = at_full_size();
    let ground = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        400.0,
        300.0,
        Rgba8::opaque(250, 250, 200),
    )
    .expect("rect");
    ws.draw_shape(ground, "ground");
    ws.draw_shape(brush_ring(), "ring");
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    let all = drawings(&ws);
    assert_eq!(all.len(), 3);
    assert_eq!(
        fill_of(&all[0]),
        Rgba8::opaque(250, 250, 200),
        "the ground is kept"
    );
    assert_eq!(fill_of(&all[1]), Rgba8::opaque(20, 40, 200), "the new area");
    // Outside the ring the ground itself is what is clicked: it is repainted whole.
    ws.click_with_tool(Pos2::new(300.0, 250.0), false);
    assert_eq!(fill_of(&drawings(&ws)[0]), Rgba8::opaque(20, 40, 200));
}

#[test]
fn the_eyedropper_takes_the_color_shown_where_no_shape_has_paint() {
    let mut ws = at_full_size();
    ws.set_stage_background(Rgba8::opaque(12, 34, 56));
    ws.set_tool(Tool::Eyedropper);
    ws.click_with_tool(Pos2::new(300.0, 300.0), false);
    assert_eq!(ws.paint.fill, Color32::from_rgb(12, 34, 56));
    ws.paint.fill = BLUE;
    ws.click_with_tool(Pos2::new(-50.0, 300.0), false);
    assert_eq!(
        ws.paint.fill, BLUE,
        "off the stage there is nothing to take"
    );
}
