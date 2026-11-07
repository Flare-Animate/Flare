//! The SVG export: a frame's shapes come out as paths with their fills, gradients and strokes, a
//! stroke SVG cannot draw as it is named in the notes, and the file imports back as shapes.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;
use std::path::Path;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{
    Content, Document, FrameSpan, Gradient, Layer, Scene, Shape, Stage, Stroke, StrokeStyle,
    Timeline,
};

/// A stage of 40 by 30 with one layer per shape, each shape visible at frame 0.
fn document(shapes: Vec<Shape>) -> Document {
    let layers = shapes
        .into_iter()
        .map(|shape| {
            let span = FrameSpan::new(0, NonZeroU32::new(1).expect("one"), Content::Shape(shape));
            Layer::new(LayerId::new(), "L", vec![span]).expect("layer")
        })
        .collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(40, 30, Rgba8::WHITE).expect("stage"))
}

#[test]
fn a_frame_is_an_svg_of_its_fills_gradients_and_strokes() {
    let red = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        10.0,
        10.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let gradient = Shape::rect(
        ShapeId::new(),
        12.0,
        0.0,
        10.0,
        10.0,
        Rgba8::opaque(0, 0, 0),
    )
    .expect("rect")
    .with_gradient(
        Gradient::linear(
            flare_core::DocumentPoint::new(12.0, 0.0).expect("point"),
            flare_core::DocumentPoint::new(22.0, 0.0).expect("point"),
            Rgba8::opaque(255, 255, 255),
            Rgba8::opaque(0, 0, 0),
        )
        .expect("gradient"),
    );
    let line = Shape::line(
        ShapeId::new(),
        (0.0, 20.0),
        (30.0, 20.0),
        Stroke::new(2.0, Rgba8::opaque(0, 0, 255)).expect("stroke"),
    )
    .expect("line");
    let svg = crate::svg_export(&document(vec![red, gradient, line]), 0).expect("svg");
    let text = &svg.text;
    assert!(text.starts_with("<svg"), "{text}");
    assert!(
        text.contains(r##"width="40" height="30""##),
        "the stage size"
    );
    assert!(
        text.contains(r##"fill="#ff0000" fill-opacity="1""##),
        "a solid fill"
    );
    assert!(
        text.contains(r##"<linearGradient id="grad0""##),
        "the gradient"
    );
    assert!(
        text.contains(r##"fill="url(#grad0)""##),
        "the gradient fill"
    );
    assert!(text.contains(r##"stroke="#0000ff""##), "the stroke");
    assert!(svg.notes.is_empty(), "nothing is left out: {:?}", svg.notes);
}

#[test]
fn a_patterned_stroke_is_drawn_solid_and_the_notes_say_so() {
    let dashed = Shape::line(
        ShapeId::new(),
        (0.0, 5.0),
        (30.0, 5.0),
        Stroke::new(1.0, Rgba8::opaque(0, 0, 0))
            .expect("stroke")
            .with_style(StrokeStyle::Dashed),
    )
    .expect("line");
    let svg = crate::svg_export(&document(vec![dashed]), 0).expect("svg");
    assert!(
        svg.text.contains("stroke-width"),
        "the stroke is still there"
    );
    assert!(
        svg.notes.iter().any(|n| n.contains("solid")),
        "a dashed stroke is named in the notes: {:?}",
        svg.notes
    );
}

#[test]
fn the_svg_imports_back_with_its_shapes() {
    let red = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        10.0,
        10.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let line = Shape::line(
        ShapeId::new(),
        (0.0, 20.0),
        (30.0, 20.0),
        Stroke::new(2.0, Rgba8::opaque(0, 0, 255)).expect("stroke"),
    )
    .expect("line");
    let svg = crate::svg_export(&document(vec![red, line]), 0).expect("svg");
    let imported = flare_import::import_foreign_bytes(Path::new("a.svg"), svg.text.into_bytes())
        .expect("the export imports back");
    let drawn =
        flare_eval::evaluate_frame(&imported.document, imported.document.scenes()[0].id(), 0)
            .expect("frame");
    assert!(
        drawn.shapes.len() >= 2,
        "the shapes come back: {}",
        drawn.shapes.len()
    );
}
