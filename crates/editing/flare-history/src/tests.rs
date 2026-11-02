#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{Content, FrameSpan, Instance, Layer, Scene, Symbol, Timeline};

use super::*;

struct Fixture {
    session: EditorSession,
    scene: SceneId,
    layer: LayerId,
    symbol: SymbolId,
}

fn fixture() -> Fixture {
    let (scene, layer, symbol) = (SceneId::new(), LayerId::new(), SymbolId::new());
    let layer_value = Layer::new(layer, "Layer 1", vec![]).expect("layer");
    let scene_value = Scene::new(scene, "Scene 1", Timeline::new(vec![layer_value]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let document =
        Document::new(DocumentId::new(), rate, vec![scene_value], vec![]).expect("document");
    Fixture {
        session: EditorSession::new(document),
        scene,
        layer,
        symbol,
    }
}

fn span(start: u32, symbol: SymbolId) -> FrameSpan {
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    FrameSpan::new(
        start,
        NonZeroU32::new(5).expect("nonzero"),
        Content::Instance(instance),
    )
}

fn add_symbol(f: &Fixture) -> Command {
    Command::AddSymbol(Symbol::new(f.symbol, "Art", Timeline::default()))
}

fn insert(f: &Fixture, start: u32) -> Command {
    Command::InsertSpan {
        scene: f.scene,
        layer: f.layer,
        span: span(start, f.symbol),
    }
}

#[test]
fn batch_applies_as_one_undo_step() {
    let mut f = fixture();
    let batch = vec![add_symbol(&f), insert(&f, 0), insert(&f, 5)];
    f.session.apply(batch).expect("apply");
    assert_eq!(f.session.document().symbols().len(), 1);
    assert_eq!(
        f.session.document().scenes()[0].timeline().layers()[0]
            .spans()
            .len(),
        2
    );
    assert!(f.session.undo());
    assert!(f.session.document().symbols().is_empty());
    assert!(!f.session.can_undo());
}

#[test]
fn failed_batch_changes_nothing() {
    let mut f = fixture();
    f.session.apply([add_symbol(&f)]).expect("first");
    let before = f.session.document().clone();
    // Second command overlaps the first span, so the whole batch must roll back.
    let batch = vec![insert(&f, 0), insert(&f, 2)];
    assert!(matches!(
        f.session.apply(batch),
        Err(EditError::Timeline(_))
    ));
    assert_eq!(f.session.document(), &before);
    f.session.undo();
    assert!(f.session.document().symbols().is_empty());
}

#[test]
fn dangling_reference_is_rejected_atomically() {
    let mut f = fixture();
    assert!(matches!(
        f.session.apply([insert(&f, 0)]),
        Err(EditError::Invalid(_))
    ));
    assert!(!f.session.can_undo());
}

#[test]
fn undo_redo_preserve_identity_and_new_edit_clears_redo() {
    let mut f = fixture();
    f.session
        .apply([add_symbol(&f), insert(&f, 0)])
        .expect("apply");
    let edited = f.session.document().clone();
    assert!(f.session.undo());
    assert!(f.session.can_redo());
    assert!(f.session.redo());
    assert_eq!(f.session.document(), &edited);
    f.session.undo();
    f.session.apply([add_symbol(&f)]).expect("new edit");
    assert!(!f.session.can_redo());
}

#[test]
fn missing_targets_report_structured_errors() {
    let mut f = fixture();
    let ghost = LayerId::new();
    let command = Command::RemoveSpan {
        scene: f.scene,
        layer: ghost,
        start: 0,
    };
    assert_eq!(
        f.session.apply([command]),
        Err(EditError::NoSuchLayer(ghost))
    );
    let command = Command::RemoveSpan {
        scene: f.scene,
        layer: f.layer,
        start: 9,
    };
    assert_eq!(f.session.apply([command]), Err(EditError::NoSuchSpan(9)));
}

fn layer_flags(f: &Fixture) -> (String, bool, bool) {
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    (
        layer.name().to_owned(),
        layer.is_visible(),
        layer.is_locked(),
    )
}

#[test]
fn layer_properties_change_atomically_and_survive_span_edits() {
    use flare_commands::LayerChange;
    let mut f = fixture();
    let (scene, layer) = (f.scene, f.layer);
    let update = |change| Command::UpdateLayer {
        scene,
        layer,
        change,
    };
    f.session
        .apply([
            update(LayerChange::Name("Background".into())),
            update(LayerChange::Visible(false)),
            update(LayerChange::Locked(true)),
        ])
        .expect("update");
    assert_eq!(layer_flags(&f), ("Background".to_owned(), false, true));
    // A span edit rebuilds the layer; its properties must not reset.
    f.session
        .apply([add_symbol(&f), insert(&f, 0)])
        .expect("edit spans");
    assert_eq!(layer_flags(&f), ("Background".to_owned(), false, true));
    f.session.undo();
    f.session.undo();
    assert_eq!(
        layer_flags(&f),
        ("Layer 1".to_owned(), true, false),
        "one undo step for all three"
    );
    let ghost = LayerId::new();
    let bad = Command::UpdateLayer {
        scene,
        layer: ghost,
        change: LayerChange::Visible(true),
    };
    assert_eq!(f.session.apply([bad]), Err(EditError::NoSuchLayer(ghost)));
}

#[test]
fn layers_can_be_moved_and_removed() {
    let mut f = fixture();
    let (scene, first) = (f.scene, f.layer);
    let (second, third) = (LayerId::new(), LayerId::new());
    let add = |id| Command::AddLayer {
        scene,
        layer: Layer::new(id, "x", vec![]).expect("layer"),
    };
    f.session.apply([add(second), add(third)]).expect("add");
    let order = |f: &Fixture| -> Vec<LayerId> {
        f.session.document().scenes()[0]
            .timeline()
            .layers()
            .iter()
            .map(Layer::id)
            .collect()
    };
    assert_eq!(order(&f), vec![first, second, third]);
    f.session
        .apply([Command::MoveLayer {
            scene,
            layer: third,
            index: 0,
        }])
        .expect("move up");
    assert_eq!(order(&f), vec![third, first, second]);
    f.session
        .apply([Command::MoveLayer {
            scene,
            layer: third,
            index: 99,
        }])
        .expect("clamped");
    assert_eq!(order(&f), vec![first, second, third]);
    f.session
        .apply([Command::RemoveLayer {
            scene,
            layer: second,
        }])
        .expect("remove");
    assert_eq!(order(&f), vec![first, third]);
    f.session.undo();
    assert_eq!(order(&f), vec![first, second, third]);
    let ghost = Command::RemoveLayer {
        scene,
        layer: LayerId::new(),
    };
    assert!(f.session.apply([ghost]).is_err());
}

#[test]
fn stage_changes_are_undoable_and_survive_other_edits() {
    use flare_core::Rgba8;
    use flare_document::Stage;
    let mut f = fixture();
    let default = f.session.document().stage();
    let stage = Stage::new(640, 360, Rgba8::opaque(10, 20, 30)).expect("stage");
    f.session
        .apply([Command::SetStage(stage)])
        .expect("set stage");
    assert_eq!(f.session.document().stage(), stage);
    // An unrelated edit rebuilds the document; the stage must not reset.
    f.session.apply([add_symbol(&f)]).expect("symbol");
    assert_eq!(f.session.document().stage(), stage);
    f.session.undo();
    f.session.undo();
    assert_eq!(f.session.document().stage(), default);
}

#[test]
fn revision_changes_with_every_document_change_but_not_on_failure() {
    let mut f = fixture();
    let start = f.session.revision();
    assert!(f.session.apply([insert(&f, 0)]).is_err());
    assert_eq!(f.session.revision(), start);
    f.session.apply([add_symbol(&f)]).expect("apply");
    let edited = f.session.revision();
    assert!(edited > start);
    f.session.undo();
    assert!(f.session.revision() > edited);
}

mod more;
