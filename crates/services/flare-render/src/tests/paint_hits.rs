//! What the paint tools find under a point: lines, visible fills, and outlines they can fill.

use super::*;
use flare_core::DocumentPoint;
use flare_document::{PathSegment, Stroke as DocStroke};

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn point(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

/// A square outline from (0, 0) to (40, 40) with no fill, its last side drawn as far as
/// `(0, end_y)`, so an `end_y` above 0 leaves that much of a gap; `close` closes it.
fn outline(end_y: f64, close: bool) -> Content {
    let mut path = vec![
        PathSegment::MoveTo(point(0.0, 0.0)),
        PathSegment::LineTo(point(40.0, 0.0)),
        PathSegment::LineTo(point(40.0, 40.0)),
        PathSegment::LineTo(point(0.0, 40.0)),
        PathSegment::LineTo(point(0.0, end_y)),
    ];
    if close {
        path.push(PathSegment::Close);
    }
    let shape = Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
        .expect("shape")
        .with_stroke(DocStroke::new(2.0, BLUE).expect("stroke"));
    Content::Shape(shape)
}

fn hits_at(content: Content, x: f64, y: f64, gap: f64) -> Vec<PaintHit> {
    let (doc, id) = scene_with(vec![span(content)], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    paint_hits(&frame, DVec2::new(x, y), gap)
}

#[test]
fn inside_an_unfilled_closed_outline_is_fillable_but_not_on_its_fill() {
    let hits = hits_at(outline(0.0, true), 20.0, 20.0, 0.0);
    assert_eq!(
        hits,
        vec![PaintHit {
            index: 0,
            on_stroke: false,
            on_fill: false,
            fillable: true,
        }]
    );
    let (doc, id) = scene_with(vec![span(outline(0.0, true))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(
        hit_test(&frame, DVec2::new(20.0, 20.0)),
        None,
        "selection still looks through an empty outline"
    );
}

#[test]
fn an_outline_left_open_encloses_its_inside_only_within_the_gap() {
    // The ends are 6 units apart.
    let near = hits_at(outline(6.0, false), 20.0, 20.0, 8.0);
    assert!(near.first().is_some_and(|hit| hit.fillable));
    let far = hits_at(outline(6.0, false), 20.0, 20.0, 4.0);
    assert!(
        far.is_empty(),
        "a wider gap leaves nothing to fill: {far:?}"
    );
    // Nonsense gaps count as none.
    assert!(hits_at(outline(6.0, false), 20.0, 20.0, f64::NAN).is_empty());
    assert!(hits_at(outline(6.0, false), 20.0, 20.0, -5.0).is_empty());
}

#[test]
fn the_stroke_is_told_apart_from_the_fill() {
    let filled = Shape::rect(ShapeId::new(), 0.0, 0.0, 40.0, 40.0, RED)
        .expect("rect")
        .with_stroke(DocStroke::new(4.0, BLUE).expect("stroke"));
    let on_edge = hits_at(Content::Shape(filled.clone()), 0.5, 20.0, 0.0);
    assert!(on_edge[0].on_stroke && on_edge[0].on_fill);
    let outside = hits_at(Content::Shape(filled.clone()), -1.5, 20.0, 0.0);
    assert!(outside[0].on_stroke && !outside[0].on_fill && !outside[0].fillable);
    let middle = hits_at(Content::Shape(filled), 20.0, 20.0, 0.0);
    assert!(!middle[0].on_stroke && middle[0].on_fill && middle[0].fillable);
}

#[test]
fn a_filled_open_path_is_hit_on_its_fill_as_it_is_drawn() {
    // A triangle drawn without closing it: rendering closes it to fill it.
    let path = vec![
        PathSegment::MoveTo(point(0.0, 0.0)),
        PathSegment::LineTo(point(40.0, 0.0)),
        PathSegment::LineTo(point(0.0, 40.0)),
    ];
    let shape = Shape::new(ShapeId::new(), path, RED).expect("shape");
    let (doc, id) = scene_with(vec![span(Content::Shape(shape))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(hit_test(&frame, DVec2::new(10.0, 10.0)), Some(0));
    assert_eq!(hit_test(&frame, DVec2::new(30.0, 30.0)), None);
    let hits = paint_hits(&frame, DVec2::new(10.0, 10.0), 0.0);
    assert!(hits[0].on_fill && hits[0].fillable);
}

#[test]
fn shapes_that_draw_nothing_and_empty_stage_are_not_found() {
    let clear = rect(0.0, 0.0, 40.0, 40.0, Rgba8::TRANSPARENT);
    assert!(hits_at(clear, 20.0, 20.0, 10.0).is_empty());
    assert!(hits_at(rect(0.0, 0.0, 40.0, 40.0, RED), 90.0, 90.0, 10.0).is_empty());
}

#[test]
fn paint_hits_list_the_topmost_first() {
    let spans = vec![
        span(outline(0.0, true)),
        span(rect(0.0, 0.0, 100.0, 100.0, RED)),
    ];
    let (doc, id) = scene_with(spans, vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let hits = paint_hits(&frame, DVec2::new(20.0, 20.0), 0.0);
    assert_eq!(hits.iter().map(|h| h.index).collect::<Vec<_>>(), vec![1, 0]);
    assert!(
        !hits[0].on_fill && hits[0].fillable,
        "the empty outline on top"
    );
    assert!(hits[1].on_fill, "the red square under it");
}
