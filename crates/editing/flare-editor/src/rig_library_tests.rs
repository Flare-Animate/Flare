#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Content, FrameSpan, Layer, Scene, Symbol, SymbolKind, Timeline};
use flare_history::EditorSession;
use flare_rig::{Bone, PoseKey};
use std::num::NonZeroU32;

use super::*;

fn session() -> (EditorSession, SymbolId) {
    let id = SymbolId::new();
    let symbol = Symbol::new(id, "Arm", Timeline::new(vec![])).with_kind(SymbolKind::MovieClip);
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![
            Layer::new(
                LayerId::new(),
                "L",
                vec![FrameSpan::new(0, NonZeroU32::MIN, Content::Empty)],
            )
            .expect("layer"),
        ]),
    );
    let document =
        flare_document::Document::new(DocumentId::new(), rate, vec![scene], vec![symbol])
            .expect("document");
    (EditorSession::new(document), id)
}

fn bone(name: &str, parent: Option<usize>) -> Bone {
    Bone {
        name: name.to_owned(),
        parent,
        length: 10.0,
        rest_angle: 0.0,
        limit: None,
        target: None,
    }
}

#[test]
fn a_rig_is_made_edited_and_removed_each_as_one_undo_step() {
    let (mut s, symbol) = session();
    let make = create(s.document(), symbol).expect("create");
    s.apply([make]).expect("apply");
    assert_eq!(s.document().rig_assets().len(), 1);
    assert_eq!(
        find(s.document(), symbol)
            .expect("read")
            .expect("rig")
            .clock,
        flare_rig::Clock::Free,
        "a movie clip runs on its own clock"
    );
    let grow = edit(
        s.document(),
        symbol,
        &[
            RigCommand::AddBone(bone("upper", None)),
            RigCommand::AddBone(bone("lower", Some(0))),
            RigCommand::SetRigPose {
                frame: 3,
                key: PoseKey::default(),
            },
        ],
    )
    .expect("edit");
    s.apply([grow]).expect("apply");
    let rig = find(s.document(), symbol).expect("read").expect("rig");
    assert_eq!((rig.bones.len(), rig.keys.len()), (2, 1));
    s.undo();
    assert!(
        find(s.document(), symbol)
            .expect("read")
            .expect("rig")
            .bones
            .is_empty()
    );
    let gone = remove(s.document(), symbol).expect("remove");
    s.apply([gone]).expect("apply");
    assert!(s.document().rig_assets().is_empty());
    s.undo();
    assert_eq!(
        s.document().rig_assets().len(),
        1,
        "one undo brings it back"
    );
}

#[test]
fn refusals_name_the_reason_and_change_nothing() {
    let (mut s, symbol) = session();
    let other = SymbolId::new();
    assert_eq!(
        create(s.document(), other),
        Err(RigLibraryError::NoSuchSymbol)
    );
    assert_eq!(
        edit(s.document(), symbol, &[]).map(|_| ()),
        Err(RigLibraryError::NoRig)
    );
    assert_eq!(
        remove(s.document(), symbol).map(|_| ()),
        Err(RigLibraryError::NoRig)
    );
    let make = create(s.document(), symbol).expect("create");
    s.apply([make]).expect("apply");
    assert_eq!(
        create(s.document(), symbol),
        Err(RigLibraryError::AlreadyRigged)
    );
    // A bad edit in a batch (a bone whose parent is not earlier) fails the whole batch.
    let bad = edit(
        s.document(),
        symbol,
        &[
            RigCommand::AddBone(bone("ok", None)),
            RigCommand::AddBone(bone("bad", Some(5))),
        ],
    );
    assert!(matches!(bad, Err(RigLibraryError::Rig(_))));
    assert!(
        find(s.document(), symbol)
            .expect("read")
            .expect("rig")
            .bones
            .is_empty()
    );
    assert!(matches!(
        parse("{ nonsense"),
        Err(RigLibraryError::NotARig(_))
    ));
    assert!(matches!(
        parse(r#"{"symbol":"x"}"#),
        Err(RigLibraryError::NotARig(_))
    ));
}

#[test]
fn a_rig_cannot_name_a_missing_symbol_and_goes_with_its_symbol() {
    let (mut s, symbol) = session();
    let make = create(s.document(), symbol).expect("create");
    s.apply([make]).expect("apply");
    let stray = flare_rig::Rig::new(SymbolId::new(), SymbolKind::Graphic);
    assert_eq!(
        set(s.document(), &stray),
        Err(RigLibraryError::NoSuchSymbol)
    );
    // The command layer refuses it too, whoever builds the command.
    let text = serde_json::to_string(&stray).expect("json");
    let asset = RigAsset::new(stray.symbol, text).expect("asset");
    assert!(s.apply([Command::SetRigAssets(vec![asset])]).is_err());
    // Removing the symbol takes its rig with it, in the same step.
    s.apply([Command::RemoveSymbol(symbol)])
        .expect("remove symbol");
    assert!(s.document().rig_assets().is_empty());
    s.undo();
    assert_eq!(s.document().rig_assets().len(), 1);
}
