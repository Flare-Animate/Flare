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

#[test]
fn dirty_state_follows_saved_content() {
    let mut f = fixture();
    assert!(!f.session.is_dirty());
    f.session.apply([add_symbol(&f)]).expect("apply");
    assert!(f.session.is_dirty());
    f.session.mark_saved();
    assert!(!f.session.is_dirty());
    f.session.undo();
    assert!(f.session.is_dirty());
    f.session.redo();
    assert!(!f.session.is_dirty());
}

#[test]
fn unsaved_content_is_dirty_until_saved() {
    let mut f = fixture();
    assert!(!f.session.is_dirty());
    f.session.mark_unsaved();
    assert!(f.session.is_dirty());
    f.session.mark_saved();
    assert!(!f.session.is_dirty());
}

#[test]
fn history_is_bounded() {
    let mut f = fixture();
    f.session = EditorSession::with_history_depth(f.session.document().clone(), 2);
    for start in [0, 5, 10] {
        let mut batch = vec![];
        if start == 0 {
            batch.push(add_symbol(&f));
        }
        batch.push(insert(&f, start));
        f.session.apply(batch).expect("apply");
    }
    assert!(f.session.undo());
    assert!(f.session.undo());
    assert!(!f.session.undo());
}

fn with_notes(f: &Fixture, layer_notes: &[(LayerId, u32)]) -> Vec<flare_document::Attachment> {
    use flare_document::{Anchor, Attachment, Placement};
    let mut all = vec![
        Attachment::new("xfl", Anchor::Document, Placement::Child, "<media/>").expect("doc note"),
        Attachment::new(
            "xfl",
            Anchor::Scene(f.scene),
            Placement::LayerAt { index: 0 },
            "<DOMLayer layerType=\"mask\"/>",
        )
        .expect("scene note"),
    ];
    for (layer, frame) in layer_notes {
        all.push(
            Attachment::new(
                "xfl",
                Anchor::Layer(*layer),
                Placement::FrameElement { frame: *frame },
                "<DOMStaticText/>",
            )
            .expect("frame note"),
        );
    }
    all
}

#[test]
fn source_attachments_survive_every_kind_of_edit() {
    let mut f = fixture();
    let doc = f.session.document().clone();
    let notes = with_notes(&f, &[(f.layer, 0)]);
    let doc = doc.with_attachments(notes.clone()).expect("attach");
    f.session = EditorSession::new(doc);

    // Span edits, layer property edits, stage edits and symbol edits all rebuild the
    // document; none may lose the attachments.
    f.session
        .apply([add_symbol(&f), insert(&f, 0)])
        .expect("edit");
    let update = |change| Command::UpdateLayer {
        scene: f.scene,
        layer: f.layer,
        change,
    };
    f.session
        .apply([update(flare_commands::LayerChange::Locked(true))])
        .expect("lock");
    let stage = flare_document::Stage::new(50, 50, flare_core::Rgba8::WHITE).expect("stage");
    f.session.apply([Command::SetStage(stage)]).expect("stage");
    assert_eq!(f.session.document().attachments(), notes.as_slice());

    // Undo and redo restore exact snapshots, attachments included.
    while f.session.undo() {}
    assert_eq!(f.session.document().attachments(), notes.as_slice());
    while f.session.redo() {}
    assert_eq!(f.session.document().attachments(), notes.as_slice());
}

#[test]
fn deleting_a_layer_removes_what_hung_off_it_and_undo_brings_it_back() {
    let mut f = fixture();
    let extra = LayerId::new();
    let add = Command::AddLayer {
        scene: f.scene,
        layer: Layer::new(extra, "extra", vec![]).expect("layer"),
    };
    f.session.apply([add]).expect("add");
    let notes = with_notes(&f, &[(f.layer, 0), (extra, 2)]);
    let doc = f
        .session
        .document()
        .clone()
        .with_attachments(notes.clone())
        .expect("attach");
    f.session = EditorSession::new(doc);

    f.session
        .apply([Command::RemoveLayer {
            scene: f.scene,
            layer: extra,
        }])
        .expect("remove");
    let left = f.session.document().attachments();
    assert_eq!(
        left.len(),
        3,
        "the removed layer's note is gone, the rest stay"
    );
    assert!(
        left.iter()
            .all(|a| a.anchor() != flare_document::Anchor::Layer(extra))
    );
    f.session.undo();
    assert_eq!(f.session.document().attachments(), notes.as_slice());
}

#[test]
fn a_snapshot_saved_later_leaves_newer_edits_dirty() {
    let mut f = fixture();
    let snapshot = f.session.snapshot();
    let rename = Command::UpdateLayer {
        scene: f.scene,
        layer: f.layer,
        change: flare_commands::LayerChange::Name("edited while saving".into()),
    };
    f.session.apply([rename]).expect("apply");
    f.session.mark_snapshot_saved(&snapshot);
    assert!(
        f.session.is_dirty(),
        "the edit made during the save is not saved"
    );
    assert!(f.session.undo());
    assert!(
        !f.session.is_dirty(),
        "undoing back to the saved content is clean"
    );
    // The snapshot is unaffected by what happened to the session since.
    assert_eq!(
        snapshot.scenes()[0].timeline().layers()[0].name(),
        "Layer 1"
    );
}

#[test]
fn symbols_can_be_renamed_and_removed_unless_an_instance_uses_them() {
    let mut f = fixture();
    f.session.apply([add_symbol(&f)]).expect("add");
    f.session
        .apply([Command::RenameSymbol {
            symbol: f.symbol,
            name: "Hero".into(),
        }])
        .expect("rename");
    assert_eq!(f.session.document().symbols()[0].name(), "Hero");
    f.session.apply([insert(&f, 0)]).expect("use it");
    assert!(
        f.session.apply([Command::RemoveSymbol(f.symbol)]).is_err(),
        "an instance still uses it"
    );
    assert_eq!(f.session.document().symbols().len(), 1);
    f.session
        .apply([Command::RemoveSpan {
            scene: f.scene,
            layer: f.layer,
            start: 0,
        }])
        .expect("clear");
    f.session
        .apply([Command::RemoveSymbol(f.symbol)])
        .expect("remove");
    assert!(f.session.document().symbols().is_empty());
    assert!(f.session.undo());
    assert_eq!(f.session.document().symbols()[0].name(), "Hero");
    assert!(
        f.session
            .apply([Command::RemoveSymbol(SymbolId::new())])
            .is_err()
    );
}

#[test]
fn the_frame_rate_is_an_undoable_edit() {
    let mut f = fixture();
    let rate = FrameRate::new(30, 1).expect("rate");
    f.session.apply([Command::SetFrameRate(rate)]).expect("set");
    assert_eq!(f.session.document().frame_rate(), rate);
    assert!(f.session.undo());
    assert_eq!(f.session.document().frame_rate().numerator(), 24);
}

#[test]
fn steps_carry_their_labels_through_undo_redo_and_the_depth_limit() {
    let mut f = fixture();
    for name in ["one", "two", "three"] {
        let layer = Layer::new(LayerId::new(), name, vec![]).expect("layer");
        f.session
            .apply_labeled(
                [Command::AddLayer {
                    scene: f.scene,
                    layer,
                }],
                &format!("Add {name}"),
            )
            .expect("apply");
    }
    assert_eq!(
        f.session.undo_labels().collect::<Vec<_>>(),
        ["Add one", "Add two", "Add three"]
    );
    assert!(f.session.undo() && f.session.undo());
    assert_eq!(f.session.undo_labels().collect::<Vec<_>>(), ["Add one"]);
    assert_eq!(
        f.session.redo_labels().collect::<Vec<_>>(),
        ["Add two", "Add three"],
        "the next redo is first"
    );
    assert!(f.session.redo());
    assert_eq!(
        f.session.undo_labels().collect::<Vec<_>>(),
        ["Add one", "Add two"]
    );
    // A new edit drops what could have been redone.
    f.session
        .apply([Command::SetStage(f.session.document().stage())])
        .expect("apply");
    assert_eq!(f.session.redo_labels().len(), 0);
    assert_eq!(f.session.undo_labels().last(), Some("Edit"));
    f.session.clear_history();
    assert!(!f.session.can_undo() && f.session.undo_labels().len() == 0);
    // The label list never outgrows a short history.
    let mut short = EditorSession::with_history_depth(f.session.document().clone(), 2);
    for i in 0..5 {
        short
            .apply_labeled(
                [Command::SetStage(short.document().stage())],
                &format!("s{i}"),
            )
            .expect("apply");
    }
    assert_eq!(short.undo_labels().collect::<Vec<_>>(), ["s3", "s4"]);
}
