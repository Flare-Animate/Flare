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

mod playback {
    use super::*;
    use flare_core::{InstanceId, Transform2D};
    use flare_document::{Content, FrameSpan, Instance, PlayMode, Playback, Tween};
    use std::num::NonZeroU32;

    fn with_instance() -> (Document, SceneId, LayerId) {
        let symbol = Symbol::new(SymbolId::new(), "S", Timeline::default());
        let instance = Instance::new(InstanceId::new(), symbol.id(), Transform2D::IDENTITY);
        let span = FrameSpan::new(
            2,
            NonZeroU32::new(5).expect("nonzero"),
            Content::Instance(instance),
        )
        .with_tween(Tween::Linear)
        .with_ease(30);
        let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
        let (layer_id, scene_id) = (layer.id(), SceneId::new());
        let scene = Scene::new(scene_id, "S", Timeline::new(vec![layer]));
        let document = Document::new(
            DocumentId::new(),
            FrameRate::new(24, 1).expect("rate"),
            vec![scene],
            vec![symbol],
        )
        .expect("document");
        (document, scene_id, layer_id)
    }

    fn set(scene: SceneId, layer: LayerId, start: u32) -> Command {
        Command::SetInstancePlayback {
            scene,
            layer,
            start,
            playback: Playback {
                mode: PlayMode::PlayOnce,
                first_frame: 3,
            },
        }
    }

    #[test]
    fn sets_playback_and_keeps_timing_and_motion() {
        let (before, scene, layer) = with_instance();
        let after = apply_all(&before, [set(scene, layer, 2)])
            .expect("applied")
            .expect("changed");
        let span = &after.scenes()[0].timeline().layers()[0].spans()[0];
        let Content::Instance(instance) = span.content() else {
            unreachable!("the span held an instance")
        };
        assert_eq!(instance.playback().mode, PlayMode::PlayOnce);
        assert_eq!(instance.playback().first_frame, 3);
        assert_eq!((span.start(), span.duration().get()), (2, 5));
        assert_eq!((span.tween(), span.ease()), (Tween::Linear, 30));
    }

    #[test]
    fn a_missing_span_fails_and_changes_nothing() {
        let (before, scene, layer) = with_instance();
        assert!(apply_all(&before, [set(scene, layer, 9)]).is_err());
    }
}
