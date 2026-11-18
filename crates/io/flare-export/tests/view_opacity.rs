//! A transparent layer is a view aid XFL cannot hold, and a layer's effects have no place in it:
//! the export says so for each.
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

#[test]
fn a_layer_with_effects_is_named_in_the_notes_and_a_plain_one_is_not() {
    let says_effects = |document: &Document| {
        export_xfl(document)
            .expect("export")
            .diagnostics
            .iter()
            .any(|n| n.message.contains("effects") && n.location.contains("Hero"))
    };
    let (graph, _) = flare_fx::EffectGraph::new()
        .append(flare_fx::EffectKind::Glow)
        .expect("add");
    let glowing = Layer::new(LayerId::new(), "Hero", vec![])
        .expect("layer")
        .with_effects(Some(graph));
    assert!(says_effects(&with_layers(vec![glowing])));
    let plain = Layer::new(LayerId::new(), "Hero", vec![]).expect("layer");
    assert!(!says_effects(&with_layers(vec![plain])));
}
