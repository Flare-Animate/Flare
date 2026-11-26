//! XFL has no blend mode on a layer and no raster layer: an export says so for each such layer,
//! and says nothing for plain layers.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, LayerBlend, Scene, Timeline};

fn document(layers: Vec<Layer>) -> Document {
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

fn notes(document: &Document) -> Vec<String> {
    flare_export::export_xfl(document)
        .expect("export")
        .diagnostics
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn a_blended_or_raster_layer_is_reported_and_a_plain_one_is_not() {
    let layers = vec![
        Layer::new(LayerId::new(), "Glow", vec![])
            .expect("layer")
            .with_blend(LayerBlend::Screen),
        Layer::new(LayerId::new(), "Paint", vec![])
            .expect("layer")
            .with_raster(true),
        Layer::new(LayerId::new(), "Ink", vec![]).expect("layer"),
    ];
    let notes = notes(&document(layers));
    assert!(
        notes.iter().any(|m| m.contains("Screen blend mode")),
        "{notes:?}"
    );
    assert!(
        notes.iter().any(|m| m.contains("no raster layers")),
        "{notes:?}"
    );
    let about = notes
        .iter()
        .filter(|m| m.contains("blend mode") || m.contains("raster"))
        .count();
    assert_eq!(about, 2, "only the two layers are named: {notes:?}");
}
