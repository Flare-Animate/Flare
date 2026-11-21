//! A layer that holds its drawing keeps it on show in the file too: Animate has no holding, so the
//! last keyframe is written as long as the scene. A reference layer is left out.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_export::{export_xfl, fla_bytes};
use flare_import::import_fla_bytes;

fn art(frames: u32) -> FrameSpan {
    let shape =
        Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::opaque(9, 9, 9)).expect("rect");
    FrameSpan::new(
        0,
        NonZeroU32::new(frames).expect("nonzero"),
        Content::Shape(shape),
    )
}

fn document(hold: bool) -> Document {
    let held = Layer::new(LayerId::new(), "Held", vec![art(4)])
        .expect("layer")
        .with_hold(hold);
    let long = Layer::new(LayerId::new(), "Long", vec![art(10)]).expect("layer");
    let reference = Layer::new(LayerId::new(), "Guide", vec![art(10)])
        .expect("layer")
        .with_reference(true);
    let scene = Scene::new(
        SceneId::new(),
        "Scene 1",
        Timeline::new(vec![held, long, reference]),
    );
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

fn trip(document: &Document) -> Vec<(String, u32)> {
    let bytes = fla_bytes(&export_xfl(document).expect("export")).expect("bytes");
    let imported = import_fla_bytes(bytes).expect("import");
    imported.document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| {
            (
                l.name().to_owned(),
                l.spans()
                    .last()
                    .map_or(0, |s| s.duration().get() + s.start()),
            )
        })
        .collect()
}

#[test]
fn a_held_drawing_is_written_as_long_as_the_scene_and_a_reference_layer_not_at_all() {
    assert_eq!(
        trip(&document(true)),
        vec![("Held".to_owned(), 10), ("Long".to_owned(), 10)]
    );
}

#[test]
fn a_layer_that_does_not_hold_ends_where_it_ends() {
    assert_eq!(trip(&document(false))[0], ("Held".to_owned(), 4));
}
