//! A keyframe of several elements goes out as one <DOMFrame> with all of them, and comes back as
//! one keyframe on one layer.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_export::{export_xfl, fla_bytes};
use flare_import::import_fla_bytes;

fn rect(x: f64) -> Content {
    Content::Shape(
        Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::opaque(200, 0, 0)).expect("rect"),
    )
}

fn document() -> Document {
    let span = FrameSpan::new(0, NonZeroU32::new(4).expect("nonzero"), rect(0.0))
        .with_extras(vec![rect(20.0), rect(40.0)])
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
fn every_element_is_written_inside_the_one_frame() {
    let output = export_xfl(&document()).expect("export");
    let timeline = output
        .files
        .iter()
        .find(|(_, body)| body.contains("<DOMLayer"))
        .expect("a file with the layer");
    assert_eq!(timeline.1.matches("<DOMFrame").count(), 1);
    assert_eq!(timeline.1.matches("<DOMShape").count(), 3);
    assert_eq!(timeline.1.matches("<DOMLayer").count(), 1);
}

#[test]
fn the_elements_come_back_together_in_their_order() {
    let output = export_xfl(&document()).expect("export");
    let back = import_fla_bytes(fla_bytes(&output).expect("bytes")).expect("import");
    let layers = back.document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 1, "no layer is made for the extra shapes");
    let span = &layers[0].spans()[0];
    assert_eq!(span.element_count(), 3);
    let lefts: Vec<f64> = span
        .elements()
        .filter_map(|c| match c {
            Content::Shape(s) => s.path().iter().find_map(|p| match p {
                flare_document::PathSegment::MoveTo(p) => Some(p.as_vec().x),
                _ => None,
            }),
            _ => None,
        })
        .collect();
    assert_eq!(lefts, vec![0.0, 20.0, 40.0]);
}
