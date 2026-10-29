//! A `.fla` made and read in memory, for a browser that has no file system.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_export::{export_xfl, fla_bytes};
use flare_import::import_fla_bytes;

fn document() -> Document {
    let shape = Shape::rect(
        ShapeId::new(),
        2.0,
        3.0,
        40.0,
        30.0,
        Rgba8::opaque(200, 10, 10),
    )
    .expect("rect");
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(6).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "Art", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn a_document_survives_the_trip_through_fla_bytes() {
    let output = export_xfl(&document()).expect("export");
    let bytes = fla_bytes(&output).expect("bytes");
    assert!(bytes.starts_with(b"PK\x03\x04"), "a zip archive");
    let imported = import_fla_bytes(bytes).expect("import");
    let layers = imported.document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name(), "Art");
    assert_eq!(layers[0].spans()[0].duration().get(), 6);
}

#[test]
fn bytes_that_are_not_a_fla_are_refused_with_a_reason() {
    assert!(import_fla_bytes(b"definitely not a zip".to_vec()).is_err());
    assert!(import_fla_bytes(Vec::new()).is_err());
}
