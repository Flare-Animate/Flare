//! Animate tells a drawing object from its merged drawing by `isDrawingObject` on the
//! `<DOMShape>`; the mark goes out on drawing objects and comes back as it went.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_export::{export_xfl, fla_bytes};
use flare_import::import_fla_bytes;

fn rect(x: f64, merged: bool) -> Content {
    Content::Shape(
        Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::opaque(200, 0, 0))
            .expect("rect")
            .merged(merged),
    )
}

fn document() -> Document {
    let span = FrameSpan::new(0, NonZeroU32::new(4).expect("nonzero"), rect(0.0, true))
        .with_extras(vec![rect(20.0, false)])
        .expect("extras");
    let layer = Layer::new(LayerId::new(), "Art", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn only_a_drawing_object_is_marked_as_one() {
    let output = export_xfl(&document()).expect("export");
    let timeline = output
        .files
        .iter()
        .find(|(_, body)| body.contains("<DOMLayer"))
        .expect("a file with the layer");
    assert_eq!(timeline.1.matches("<DOMShape").count(), 2);
    assert_eq!(timeline.1.matches("isDrawingObject=\"true\"").count(), 1);
}

#[test]
fn merged_drawing_and_drawing_objects_come_back_as_they_went() {
    let output = export_xfl(&document()).expect("export");
    let back = import_fla_bytes(fla_bytes(&output).expect("bytes")).expect("import");
    let span = &back.document.scenes()[0].timeline().layers()[0].spans()[0];
    let merged: Vec<bool> = span
        .elements()
        .map(|c| matches!(c, Content::Shape(s) if s.is_merged()))
        .collect();
    assert_eq!(merged, [true, false]);
}
