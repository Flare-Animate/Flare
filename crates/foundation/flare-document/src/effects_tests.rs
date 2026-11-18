#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SymbolId};
use flare_fx::{EffectGraph, EffectKind};

use crate::{Diagnostic, Document, Layer, Scene, Symbol, Timeline};

fn shadowed() -> EffectGraph {
    EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add")
        .0
}

fn document(scene: Vec<Layer>, symbol: Vec<Layer>) -> Result<Document, Vec<Diagnostic>> {
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(scene));
    let symbol = Symbol::new(SymbolId::new(), "Ball", Timeline::new(symbol));
    Document::new(DocumentId::new(), rate, vec![scene], vec![symbol])
}

fn layer() -> Layer {
    Layer::new(LayerId::new(), "Layer", vec![]).expect("layer")
}

#[test]
fn effects_are_kept_when_the_layer_is_rebuilt() {
    let with = layer().with_effects(Some(shadowed()));
    assert_eq!(with.effects(), Some(&shadowed()));
    let rebuilt = with.with_spans(vec![]).expect("spans");
    assert_eq!(rebuilt.effects(), Some(&shadowed()));
    assert_eq!(rebuilt.with_effects(None).effects(), None);
}

#[test]
fn the_plain_graph_is_no_effects_but_one_being_built_is_kept() {
    assert_eq!(
        layer().with_effects(Some(EffectGraph::new())).effects(),
        None
    );
    let (unconnected, _) = EffectGraph::new()
        .add(EffectKind::Blur, [10.0, 10.0])
        .expect("add");
    assert!(unconnected.is_identity());
    let kept = layer().with_effects(Some(unconnected.clone()));
    assert_eq!(kept.effects(), Some(&unconnected));
}

#[test]
fn effects_go_on_a_scene_layer_that_is_drawn() {
    assert!(document(vec![layer().with_effects(Some(shadowed()))], vec![]).is_ok());
    for special in [
        layer().with_mask(true),
        layer().with_guide(true),
        layer().with_camera(true),
        layer().with_folder(true),
    ] {
        let special = special.with_effects(Some(shadowed()));
        let id = special.id();
        let errors = document(vec![special], vec![]).expect_err("refused");
        assert!(
            errors.contains(&Diagnostic::EffectsOnSpecialLayer(id)),
            "{errors:?}"
        );
    }
    let inside = layer().with_effects(Some(shadowed()));
    let id = inside.id();
    let errors = document(vec![layer()], vec![inside]).expect_err("refused");
    assert_eq!(errors, vec![Diagnostic::EffectsInSymbol(id)]);
}
