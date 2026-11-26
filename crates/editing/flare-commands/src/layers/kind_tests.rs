#![allow(clippy::expect_used)]

use super::*;
use crate::apply_all;
use flare_core::{DocumentId, FrameRate};
use flare_document::{LayerBlend, Scene, Timeline};

fn build(layers: Vec<Layer>) -> (Document, SceneId) {
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(layers))],
        vec![],
    )
    .expect("document");
    (document, scene)
}

fn first(document: &Document) -> &Layer {
    &document.scenes()[0].timeline().layers()[0]
}

#[test]
fn a_new_layer_is_a_vector_layer_unless_asked_and_needs_a_name() {
    let vector = new_layer(LayerId::new(), "Ink", false).expect("layer");
    // A keyframe on a new layer shows for its own frames, not to the end of the others.
    assert!(!vector.is_raster() && !vector.is_hold());
    assert!(
        new_layer(LayerId::new(), "Paint", true)
            .expect("layer")
            .is_raster()
    );
    assert_eq!(
        new_layer(LayerId::new(), "  ", true),
        Err(LayerRefusal::NeedsAName)
    );
}

#[test]
fn blend_and_kind_change_as_one_step_and_the_same_value_changes_nothing() {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let id = layer.id();
    let (document, scene) = build(vec![layer]);
    let commands = set_blend(&document, scene, id, LayerBlend::Overlay).expect("blend");
    let document = apply_all(&document, commands)
        .expect("valid")
        .expect("changed");
    assert_eq!(first(&document).blend(), LayerBlend::Overlay);
    assert!(
        set_blend(&document, scene, id, LayerBlend::Overlay)
            .expect("same")
            .is_empty()
    );
    let commands = set_raster(&document, scene, id, true).expect("raster");
    let document = apply_all(&document, commands)
        .expect("valid")
        .expect("changed");
    assert!(first(&document).is_raster());
    assert!(
        set_raster(&document, scene, id, true)
            .expect("same")
            .is_empty()
    );
    assert_eq!(
        set_blend(&document, scene, LayerId::new(), LayerBlend::Screen),
        Err(LayerRefusal::NoSuchLayer)
    );
}

#[test]
fn a_layer_that_draws_nothing_takes_no_blend_and_cannot_be_raster() {
    let folder = Layer::new(LayerId::new(), "F", vec![])
        .expect("layer")
        .with_folder(true);
    let id = folder.id();
    let (document, scene) = build(vec![folder]);
    assert!(matches!(
        set_blend(&document, scene, id, LayerBlend::Multiply),
        Err(LayerRefusal::Invalid(_))
    ));
    assert!(matches!(
        set_raster(&document, scene, id, true),
        Err(LayerRefusal::Invalid(_))
    ));
    assert!(
        set_blend(&document, scene, id, LayerBlend::Normal)
            .expect("normal is always fine")
            .is_empty()
    );
}
