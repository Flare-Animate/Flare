#![allow(clippy::expect_used)]

use flare_commands::{Command, apply_all};
use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SymbolId};
use flare_document::{Document, Layer, Scene, Symbol, SymbolKind, Timeline};

fn document(symbol: &Symbol) -> Document {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![symbol.clone()],
    )
    .expect("document")
}

#[test]
fn setting_a_symbols_kind_changes_only_the_kind() {
    let id = SymbolId::new();
    let symbol = Symbol::new(id, "Ball", Timeline::default());
    let before = document(&symbol);
    let after = apply_all(
        &before,
        [Command::SetSymbolKind {
            symbol: id,
            kind: SymbolKind::MovieClip,
        }],
    )
    .expect("applied")
    .expect("changed");
    let changed = after.symbol(id).expect("symbol");
    assert_eq!(changed.kind(), SymbolKind::MovieClip);
    assert_eq!(changed.name(), "Ball");
    assert_eq!(
        before.symbol(id).expect("symbol").kind(),
        SymbolKind::Graphic
    );
}

#[test]
fn a_symbol_that_is_not_there_is_refused_and_nothing_changes() {
    let symbol = Symbol::new(SymbolId::new(), "Ball", Timeline::default());
    let before = document(&symbol);
    let result = apply_all(
        &before,
        [Command::SetSymbolKind {
            symbol: SymbolId::new(),
            kind: SymbolKind::Button,
        }],
    );
    assert!(result.is_err());
}

#[test]
fn replacing_a_symbols_timeline_keeps_its_kind() {
    let id = SymbolId::new();
    let symbol = Symbol::new(id, "Ball", Timeline::default()).with_kind(SymbolKind::Button);
    let after = apply_all(
        &document(&symbol),
        [Command::ReplaceSymbolTimeline {
            symbol: id,
            timeline: Timeline::default(),
        }],
    )
    .expect("applied")
    .expect("changed");
    assert_eq!(after.symbol(id).expect("symbol").kind(), SymbolKind::Button);
}
