use super::*;
use crate::records::RIG_SCHEMA_VERSION;
use flare_document::RigAsset;
use flare_rig::{Bone, Rig, RigCommand};

fn rigged() -> Document {
    let document = sample();
    let symbol = document.symbols()[0].id();
    let rig = Rig::new(symbol, document.symbols()[0].kind())
        .apply(&[RigCommand::AddBone(Bone {
            name: "arm".to_owned(),
            parent: None,
            length: 12.0,
            rest_angle: 0.5,
            limit: None,
            target: None,
        })])
        .expect("rig");
    let text = serde_json::to_string(&rig).expect("json");
    document
        .with_rig_assets(vec![RigAsset::new(symbol, text).expect("asset")])
        .expect("rigs")
}

#[test]
fn rigs_survive_save_and_open_and_select_schema_six() {
    let document = rigged();
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("rig.flare");
    save(&document, &path).expect("save");
    assert_eq!(load(&path).expect("load"), document);
    let parts = to_parts(&document, &mut NativeZstd::new().expect("zstd")).expect("parts");
    assert_eq!(parts.schema_version(), RIG_SCHEMA_VERSION);
    assert!(
        rows(&path).iter().any(|r| r.kind == "rig"),
        "one rig record"
    );
}

#[test]
fn a_rig_record_that_is_not_a_valid_rig_for_its_symbol_is_corrupt() {
    let symbol = SymbolId::new();
    let other = SymbolId::new();
    let rig =
        serde_json::to_string(&Rig::new(other, flare_document::SymbolKind::Graphic)).expect("json");
    let wrong_symbol = wire::RigWire {
        symbol,
        payload: rig,
    };
    assert!(matches!(
        wrong_symbol.build(),
        Err(StorageError::Corrupt(_))
    ));
    let garbage = wire::RigWire {
        symbol,
        payload: "{ not json".to_owned(),
    };
    assert!(matches!(garbage.build(), Err(StorageError::Corrupt(_))));
}

#[test]
fn a_stop_motion_session_survives_save_and_open_and_selects_schema_seven() {
    use crate::records::STOP_MOTION_SCHEMA_VERSION;
    use flare_core::BitmapId;
    use flare_document::{Bitmap, StopFrame, StopMotion};
    let picture = Bitmap::new(BitmapId::new(), "f", 2, 2, vec![7; 16]).expect("bitmap");
    let id = picture.id();
    let session = StopMotion::new(12, 2)
        .and_then(|s| s.with_frames(vec![StopFrame::new(id, 3).expect("frame")]))
        .expect("session");
    let document = sample()
        .with_bitmaps(vec![picture])
        .expect("bitmaps")
        .with_stop_motion(Some(session))
        .expect("session");
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("stop.flare");
    save(&document, &path).expect("save");
    assert_eq!(load(&path).expect("load"), document);
    let parts = to_parts(&document, &mut NativeZstd::new().expect("zstd")).expect("parts");
    assert_eq!(parts.schema_version(), STOP_MOTION_SCHEMA_VERSION);
    // A session naming a picture the project lacks is not a project.
    let wire = wire::StopMotionWire::of(document.stop_motion().expect("session"));
    assert!(wire.build().is_ok());
    assert!(
        sample()
            .with_stop_motion(document.stop_motion().cloned())
            .is_err()
    );
}
