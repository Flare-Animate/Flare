//! A document's rigs: bounded, one for a symbol, and for a symbol that exists.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, SceneId, SymbolId};
use flare_document::{
    Diagnostic, Document, MAX_RIG_ASSETS, MAX_RIG_PAYLOAD_BYTES, RigAsset, RigAssetError, Scene,
    Symbol, Timeline,
};

fn document(symbols: &[SymbolId]) -> Document {
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![]))],
        symbols
            .iter()
            .map(|id| Symbol::new(*id, format!("{id:?}"), Timeline::new(vec![])))
            .collect(),
    )
    .expect("document")
}

fn rig(symbol: SymbolId) -> RigAsset {
    RigAsset::new(symbol, "{}").expect("asset")
}

#[test]
fn rigs_are_one_per_existing_symbol() {
    let a = SymbolId::new();
    let d = document(&[a]);
    assert!(d.clone().with_rig_assets(vec![rig(a)]).is_ok());
    let twice = d
        .clone()
        .with_rig_assets(vec![rig(a), rig(a)])
        .expect_err("two");
    assert!(
        twice
            .iter()
            .any(|x| matches!(x, Diagnostic::DuplicateRig(_)))
    );
    let stray = d
        .with_rig_assets(vec![rig(SymbolId::new())])
        .expect_err("stray");
    assert!(
        stray
            .iter()
            .any(|x| matches!(x, Diagnostic::DanglingRig(_)))
    );
}

#[test]
fn the_count_and_the_size_of_rigs_are_bounded() {
    let ids: Vec<SymbolId> = (0..=MAX_RIG_ASSETS).map(|_| SymbolId::new()).collect();
    let d = document(&ids);
    let all: Vec<RigAsset> = ids.iter().map(|id| rig(*id)).collect();
    let refused = d.with_rig_assets(all).expect_err("too many");
    assert!(refused.iter().any(|x| matches!(x, Diagnostic::TooManyRigs)));
    let id = SymbolId::new();
    assert_eq!(
        RigAsset::new(id, "x".repeat(MAX_RIG_PAYLOAD_BYTES + 1)),
        Err(RigAssetError::TooLarge)
    );
    assert_eq!(RigAsset::new(id, "  "), Err(RigAssetError::Empty));
}
