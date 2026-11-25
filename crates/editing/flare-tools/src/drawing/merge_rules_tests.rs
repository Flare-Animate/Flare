#![allow(clippy::expect_used)]

use super::*;
use flare_document::{Stroke, TextSource};

const RED: Rgba8 = Rgba8::new(220, 0, 0, 255);
const BLUE: Rgba8 = Rgba8::new(0, 0, 220, 255);
const BLACK: Rgba8 = Rgba8::new(0, 0, 0, 255);

/// A drawing object: what merging is asked never to touch.
fn object(x: f64, y: f64, size: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, y, size, size, fill).expect("rect")
}

/// A square drawn as merged drawing.
fn square(x: f64, y: f64, size: f64, fill: Rgba8) -> Shape {
    object(x, y, size, fill).merged(true)
}

fn line(from: (f64, f64), to: (f64, f64)) -> Shape {
    Shape::line(
        ShapeId::new(),
        from,
        to,
        Stroke::new(2.0, BLACK).expect("stroke"),
    )
    .expect("line")
    .merged(true)
}

fn merged(elements: &[Content]) -> Vec<&Shape> {
    elements
        .iter()
        .filter_map(|e| match e {
            Content::Shape(s) if s.is_merged() => Some(s),
            _ => None,
        })
        .collect()
}

/// Whether `shape`'s fill paints the point, by the non-zero rule the renderer fills with.
fn paints(shape: &Shape, x: f64, y: f64) -> bool {
    if shape.fill_is_invisible() {
        return false;
    }
    let mut winding = 0;
    for contour in areas(shape) {
        for (k, a) in contour.iter().enumerate() {
            let b = contour[(k + 1) % contour.len()];
            if (a[1] <= y) != (b[1] <= y) {
                let cross = a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                if cross > x {
                    winding += if b[1] > a[1] { 1 } else { -1 };
                }
            }
        }
    }
    winding != 0
}

/// The topmost element whose fill paints the point.
fn top_at(elements: &[Content], x: f64, y: f64) -> Option<Rgba8> {
    elements.iter().rev().find_map(|e| match e {
        Content::Shape(s) if paints(s, x, y) => Some(s.fill()),
        _ => None,
    })
}

/// How many merged fills paint the point.
fn fills_at(elements: &[Content], x: f64, y: f64) -> usize {
    merged(elements).iter().filter(|s| paints(s, x, y)).count()
}

#[test]
fn a_fill_cuts_away_the_part_of_another_colour_it_covers() {
    let first = merge_into(Vec::new(), vec![square(0.0, 0.0, 100.0, RED)]).expect("merged");
    let both = merge_into(first.elements, vec![square(50.0, 50.0, 100.0, BLUE)]).expect("merged");
    assert_eq!(both.elements.len(), 2);
    assert_eq!(top_at(&both.elements, 25.0, 25.0), Some(RED), "red left");
    assert_eq!(top_at(&both.elements, 75.0, 75.0), Some(BLUE));
    assert_eq!(
        fills_at(&both.elements, 75.0, 75.0),
        1,
        "nothing red is left under the blue"
    );
    assert_eq!(both.placed, vec![1], "the new fill is on top");
}

#[test]
fn a_fill_of_the_same_colour_that_overlaps_joins_into_one() {
    let first = merge_into(Vec::new(), vec![square(0.0, 0.0, 100.0, RED)]).expect("merged");
    let both = merge_into(first.elements, vec![square(50.0, 50.0, 100.0, RED)]).expect("merged");
    assert_eq!(both.elements.len(), 1, "one fill");
    assert_eq!(both.placed, vec![0]);
    let Content::Shape(one) = &both.elements[0] else {
        panic!("a shape");
    };
    assert!(one.is_merged());
    assert!(paints(one, 25.0, 25.0) && paints(one, 125.0, 125.0));
    assert!(!paints(one, 125.0, 25.0), "the union, not its box");
}

#[test]
fn fills_of_the_same_colour_apart_stay_two() {
    let first = merge_into(Vec::new(), vec![square(0.0, 0.0, 10.0, RED)]).expect("merged");
    let both = merge_into(first.elements, vec![square(50.0, 0.0, 10.0, RED)]).expect("merged");
    assert_eq!(both.elements.len(), 2);
}

#[test]
fn a_shape_with_a_line_becomes_a_fill_and_a_line() {
    let shape = square(0.0, 0.0, 50.0, RED).with_stroke(Stroke::new(3.0, BLACK).expect("stroke"));
    let id = shape.id();
    let drawn = merge_into(Vec::new(), vec![shape]).expect("merged");
    let pieces = merged(&drawn.elements);
    assert_eq!(pieces.len(), 2);
    assert_eq!(drawn.placed, vec![0, 1]);
    assert!(pieces[0].stroke().is_none() && pieces[0].fill() == RED);
    assert_eq!(pieces[0].id(), id, "the fill keeps the shape's identity");
    assert!(pieces[1].fill_is_invisible() && pieces[1].stroke().is_some());
    assert_ne!(pieces[1].id(), id, "the line is an element of its own");
}

#[test]
fn a_fill_cuts_the_line_under_it_and_a_line_cuts_nothing() {
    let under = merge_into(Vec::new(), vec![line((0.0, 50.0), (200.0, 50.0))]).expect("merged");
    let over = merge_into(under.elements, vec![square(50.0, 0.0, 100.0, RED)]).expect("merged");
    let lines: Vec<_> = merged(&over.elements)
        .into_iter()
        .filter(|s| s.stroke().is_some())
        .collect();
    assert_eq!(lines.len(), 1, "one line, in two stretches");
    let inside = lines[0].path().iter().any(|segment| match segment {
        flare_document::PathSegment::MoveTo(p) | flare_document::PathSegment::LineTo(p) => {
            p.as_vec().x > 51.0 && p.as_vec().x < 149.0
        }
        _ => false,
    });
    assert!(!inside, "nothing of the line is left under the fill");

    let fill = merge_into(Vec::new(), vec![square(0.0, 0.0, 100.0, RED)]).expect("merged");
    let Content::Shape(before) = fill.elements[0].clone() else {
        panic!("a shape");
    };
    let crossed =
        merge_into(fill.elements, vec![line((-10.0, 50.0), (110.0, 50.0))]).expect("merged");
    assert_eq!(crossed.elements[0], Content::Shape(before), "untouched");
    assert_eq!(crossed.placed, vec![1]);
}

#[test]
fn drawing_objects_are_never_cut_and_stay_above_the_merged_drawing() {
    let object = Content::Shape(object(0.0, 0.0, 100.0, RED));
    let drawn =
        merge_into(vec![object.clone()], vec![square(50.0, 50.0, 100.0, BLUE)]).expect("merged");
    assert_eq!(drawn.elements.len(), 2);
    assert_eq!(drawn.elements[1], object, "the object, whole and on top");
    assert_eq!(drawn.placed, vec![0], "the merged drawing goes beneath it");
    assert_eq!(top_at(&drawn.elements, 75.0, 75.0), Some(RED));
}

#[test]
fn a_gradient_never_joins_a_fill_of_its_colour() {
    let gradient = flare_document::Gradient::linear(
        flare_core::DocumentPoint::new(0.0, 0.0).expect("point"),
        flare_core::DocumentPoint::new(100.0, 0.0).expect("point"),
        RED,
        BLUE,
    )
    .expect("gradient");
    let first = merge_into(Vec::new(), vec![square(0.0, 0.0, 100.0, RED)]).expect("merged");
    let both = merge_into(
        first.elements,
        vec![square(50.0, 50.0, 100.0, RED).with_gradient(gradient)],
    )
    .expect("merged");
    assert_eq!(both.elements.len(), 2, "cut, not joined");
    assert_eq!(fills_at(&both.elements, 75.0, 75.0), 1);
}

#[test]
fn text_stays_a_drawing_object() {
    let source = TextSource::new(
        "Hi",
        "sans-serif",
        12.0,
        false,
        false,
        flare_core::DocumentPoint::new(10.0, 10.0).expect("point"),
    )
    .expect("text");
    let fill = merge_into(Vec::new(), vec![square(0.0, 0.0, 100.0, RED)]).expect("merged");
    let text = square(0.0, 0.0, 100.0, BLUE).with_text(source);
    let drawn = merge_into(fill.elements, vec![text]).expect("merged");
    assert_eq!(drawn.elements.len(), 2);
    assert_eq!(fills_at(&drawn.elements, 50.0, 50.0), 1, "nothing was cut");
    let Content::Shape(text) = &drawn.elements[1] else {
        panic!("a shape");
    };
    assert!(!text.is_merged());
    assert_eq!(drawn.placed, vec![1]);
}

#[test]
fn an_empty_keyframe_takes_the_drawing_as_it_is() {
    let drawn =
        merge_into(vec![Content::Empty], vec![square(0.0, 0.0, 10.0, RED)]).expect("merged");
    assert_eq!(drawn.elements.len(), 1);
    assert_eq!(drawn.placed, vec![0]);
}

#[test]
fn a_drawing_object_drawn_with_merged_shapes_stays_an_object() {
    let under = merge_into(Vec::new(), vec![square(0.0, 0.0, 100.0, RED)]).expect("merged");
    let drawn = merge_into(
        under.elements,
        vec![object(50.0, 50.0, 100.0, RED), square(50.0, 0.0, 20.0, RED)],
    )
    .expect("merged");
    let Some(Content::Shape(top)) = drawn.elements.last() else {
        panic!("a shape");
    };
    assert!(!top.is_merged(), "still a drawing object");
    assert_eq!(top.path().len(), 5, "whole, not joined or cut");
    assert_eq!(
        merged(&drawn.elements).len(),
        1,
        "the merged squares joined"
    );
    assert_eq!(drawn.placed, vec![0, 1], "both shapes drawn now are named");
}
