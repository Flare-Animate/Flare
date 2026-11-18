use flare_fx::{EffectGraph, EffectKind};

use super::*;

#[test]
fn a_layer_keeps_its_effects_through_save_and_load_and_a_plain_one_has_none() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let (graph, _) = EffectGraph::new().append(EffectKind::Glow).expect("add");
    let with = Layer::new(LayerId::new(), "Glowing", vec![])
        .expect("layer")
        .with_effects(Some(graph.clone()));
    let without = Layer::new(LayerId::new(), "Plain", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![with, without]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, document);
    let layers = loaded.scenes()[0].timeline().layers();
    assert_eq!(layers[0].effects(), Some(&graph));
    assert_eq!(layers[1].effects(), None);
}

#[test]
fn effects_change_the_fingerprint() {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let id = layer.id();
    let doc = |layer: Layer| {
        Document::new(
            DocumentId::new(),
            FrameRate::new(24, 1).expect("rate"),
            vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
            vec![],
        )
        .expect("document")
    };
    let (graph, _) = EffectGraph::new().append(EffectKind::Blur).expect("add");
    let plain = crate::fingerprint(&doc(layer.clone())).expect("fingerprint");
    let blurred = crate::fingerprint(&doc(layer.with_effects(Some(graph)))).expect("fingerprint");
    assert_ne!(
        plain.main, blurred.main,
        "an edit of {id:?}'s effects is an edit"
    );
}
