//! A transparent layer is a view aid XFL cannot hold: the export says so and draws nothing
//! differently.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Timeline};
use flare_export::export_xfl;

fn with_layers(layers: Vec<Layer>) -> Document {
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(layers))],
        vec![],
    )
    .expect("document")
}

fn says_opacity(document: &Document) -> bool {
    export_xfl(document)
        .expect("export")
        .diagnostics
        .iter()
        .any(|n| n.message.contains("50%") && n.location.contains("Rough"))
}

#[test]
fn a_transparent_layer_is_named_in_the_notes_and_an_opaque_one_is_not() {
    let rough = Layer::new(LayerId::new(), "Rough", vec![])
        .expect("layer")
        .with_view_opacity(50);
    assert!(says_opacity(&with_layers(vec![rough])));
    let plain = Layer::new(LayerId::new(), "Rough", vec![]).expect("layer");
    assert!(!says_opacity(&with_layers(vec![plain])));
}
