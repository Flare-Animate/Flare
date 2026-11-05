use super::*;
use flare_core::BitmapId;
use flare_document::{Bitmap, BitmapInstance};

fn with_bitmap() -> (Document, BitmapId) {
    let bitmap = Bitmap::new(BitmapId::new(), "pic", 3, 2, (0..24).collect()).expect("bitmap");
    let id = bitmap.id();
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(2).expect("nz"),
        Content::Bitmap(
            BitmapInstance::new(InstanceId::new(), id, Transform2D::IDENTITY).with_opacity(0.25),
        ),
    );
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_bitmaps(vec![bitmap])
    .expect("bitmaps");
    (document, id)
}

#[test]
fn bitmaps_and_their_instances_round_trip() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("b.flare");
    let (document, id) = with_bitmap();
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, document);
    assert_eq!(loaded.bitmap(id).expect("kept").pixels().len(), 24);
}

#[test]
fn only_files_with_bitmaps_need_the_newer_schema() {
    let dir = tempfile::tempdir().expect("dir");
    let (with, _) = with_bitmap();
    for (document, expected) in [(with, 2_i64), (sample(), 1)] {
        let path = dir.path().join(format!("v{expected}.flare"));
        save(&document, &path).expect("save");
        let conn = Connection::open(&path).expect("open");
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version");
        assert_eq!(version, expected);
        assert_eq!(load(&path).expect("load"), document);
    }
}

#[test]
fn a_build_that_knows_only_version_one_would_name_the_version() {
    // The refusal an older build gives is the same error this build gives for a later version.
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("n.flare");
    let (document, _) = with_bitmap();
    save(&document, &path).expect("save");
    Connection::open(&path)
        .expect("open")
        .pragma_update(None, "user_version", 9)
        .expect("set");
    let error = load(&path).expect_err("refused");
    assert!(error.to_string().contains("newer Flare"), "{error}");
}

#[test]
fn a_bitmap_record_whose_pixels_do_not_match_its_size_is_refused() {
    let (document, id) = with_bitmap();
    let mut raw =
        crate::wire::BitmapMeta::frame(document.bitmap(id).expect("bitmap")).expect("frame");
    raw.truncate(raw.len() - 4);
    let err = crate::wire::BitmapMeta::unframe(&id.as_uuid().to_string(), &raw);
    assert!(matches!(err, Err(StorageError::Corrupt(_))));
    assert!(crate::wire::BitmapMeta::unframe(&id.as_uuid().to_string(), &[1, 2]).is_err());
}
