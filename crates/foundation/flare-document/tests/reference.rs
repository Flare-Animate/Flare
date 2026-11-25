//! A reference layer is a stage-only picture, so a symbol may not hold one.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SymbolId};
use flare_document::{Diagnostic, Document, Layer, Scene, Symbol, Timeline};

fn document(
    scene_layers: Vec<Layer>,
    symbol_layers: Vec<Layer>,
) -> Result<Document, Vec<Diagnostic>> {
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(scene_layers))],
        vec![Symbol::new(
            SymbolId::new(),
            "Sym",
            Timeline::new(symbol_layers),
        )],
    )
}

fn layer(reference: bool) -> Layer {
    Layer::new(LayerId::new(), "L", vec![])
        .expect("layer")
        .with_reference(reference)
}

#[test]
fn a_reference_layer_in_a_symbol_is_refused() {
    let found = document(vec![], vec![layer(true)]).expect_err("reference in a symbol");
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Diagnostic::ReferenceInSymbol(_)))
    );
}

#[test]
fn a_reference_layer_in_a_scene_and_plain_layers_in_a_symbol_are_fine() {
    assert!(document(vec![layer(true)], vec![layer(false)]).is_ok());
}
