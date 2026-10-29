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

#[test]
fn stroke_ends_and_corners_round_trip_through_xfl() {
    use flare_document::{Cap, Join, Stroke};
    let stroke = Stroke::new(4.0, Rgba8::opaque(0, 0, 0))
        .expect("stroke")
        .with_cap(Cap::Butt)
        .with_join(Join::Bevel);
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 20.0, 10.0, Rgba8::TRANSPARENT)
        .expect("rect")
        .with_stroke(stroke);
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    let output = export_xfl(&document).expect("export");
    assert!(
        output
            .files
            .iter()
            .any(|(_, body)| body.contains("caps=\"none\"") && body.contains("joints=\"bevel\"")),
        "XFL's own names"
    );
    let imported = import_fla_bytes(fla_bytes(&output).expect("bytes")).expect("import");
    let back = imported.document.scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .find_map(|s| match s.content() {
            Content::Shape(shape) => shape.stroke(),
            _ => None,
        })
        .expect("a stroked shape");
    assert_eq!((back.cap(), back.join()), (Cap::Butt, Join::Bevel));
}

#[test]
fn folders_round_trip_through_xfl() {
    use flare_document::Layer as L;
    let folder = L::new(LayerId::new(), "Group", vec![])
        .expect("layer")
        .with_folder(true);
    let inside = L::new(LayerId::new(), "Inside", vec![])
        .expect("layer")
        .with_parent(Some(folder.id()));
    let outside = L::new(LayerId::new(), "Outside", vec![]).expect("layer");
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![folder, inside, outside]),
    );
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    let output = export_xfl(&document).expect("export");
    assert!(
        output
            .files
            .iter()
            .any(|(_, b)| b.contains("layerType=\"folder\""))
    );
    let back = import_fla_bytes(fla_bytes(&output).expect("bytes")).expect("import");
    let layers = back.document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 3);
    assert!(layers[0].is_folder());
    assert_eq!(layers[1].parent(), Some(layers[0].id()));
    assert_eq!(layers[2].parent(), None);
}

#[test]
fn motion_guides_round_trip_through_xfl() {
    use flare_document::Layer as L;
    let guide = L::new(LayerId::new(), "Path", vec![])
        .expect("layer")
        .with_guide(true);
    let below = L::new(LayerId::new(), "Mover", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![guide, below]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    let output = export_xfl(&document).expect("export");
    assert!(
        output
            .files
            .iter()
            .any(|(_, b)| b.contains("layerType=\"guide\""))
    );
    let back = import_fla_bytes(fla_bytes(&output).expect("bytes")).expect("import");
    let layers = back.document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    assert!(layers[0].is_guide() && !layers[1].is_guide());
    assert!(
        !back
            .diagnostics
            .iter()
            .any(|d| d.message.contains("membership")),
        "the layer below a guide pointing at it is not worth a note"
    );
}
