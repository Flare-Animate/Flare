//! Bitmaps kept as photos (JPEG) beside lossless ones.

use super::records::{put_record, rows};
use super::*;
use flare_core::BitmapId;
use flare_document::{Bitmap, BitmapInstance};

/// A smooth 96x64 picture, as a frame of video is; `alpha` everywhere.
fn picture(name: &str, alpha: u8) -> Bitmap {
    let rgba = (0..64u32)
        .flat_map(|y| (0..96u32).flat_map(move |x| [(x * 2) as u8, (y * 3) as u8, 90, alpha]))
        .collect();
    Bitmap::new(BitmapId::new(), name, 96, 64, rgba).expect("bitmap")
}

/// A document showing three bitmaps: a photo, a lossless one between, and a see-through photo.
fn mixed() -> Document {
    let bitmaps = vec![
        picture("first", 255).as_photo(85).expect("photo"),
        picture("plain", 255),
        picture("clear", 128).as_photo(60).expect("photo"),
    ];
    let spans = bitmaps
        .iter()
        .enumerate()
        .map(|(i, b)| {
            FrameSpan::new(
                i as u32,
                NonZeroU32::new(1).expect("nz"),
                Content::Bitmap(BitmapInstance::new(
                    InstanceId::new(),
                    b.id(),
                    Transform2D::IDENTITY,
                )),
            )
        })
        .collect();
    let layer = Layer::new(LayerId::new(), "L", spans).expect("layer");
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_bitmaps(bitmaps)
    .expect("bitmaps")
}

#[test]
fn photos_and_lossless_bitmaps_round_trip_in_their_order() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let document = mixed();
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(
        loaded, document,
        "the same bitmaps, photos still photos, in the same order"
    );
    let names: Vec<&str> = loaded.bitmaps().iter().map(Bitmap::name).collect();
    assert_eq!(names, ["first", "plain", "clear"]);
    assert_eq!(loaded.bitmaps()[2].photo().map(|p| p.quality()), Some(60));
    let kinds: Vec<String> = rows(&path)
        .into_iter()
        .filter(|r| r.kind == "photo" || r.kind == "bitmap")
        .map(|r| r.kind)
        .collect();
    assert_eq!(kinds.iter().filter(|k| *k == "photo").count(), 2);
}

#[test]
fn a_photo_record_is_much_smaller_than_its_pixels() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("s.flare");
    save(&mixed(), &path).expect("save");
    let sizes: Vec<(String, usize)> = rows(&path)
        .into_iter()
        .filter(|r| r.kind == "photo" || r.kind == "bitmap")
        .map(|r| (r.kind, r.record.len()))
        .collect();
    let plain = sizes.iter().find(|(k, _)| k == "bitmap").expect("plain").1;
    for (_, size) in sizes.iter().filter(|(k, _)| k == "photo") {
        assert!(
            *size < plain,
            "a photo record ({size}) is smaller than a pixel one ({plain})"
        );
    }
}

#[test]
fn a_file_with_a_photo_needs_the_photo_schema_and_others_do_not() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("v.flare");
    save(&mixed(), &path).expect("save");
    let version = |path: &std::path::Path| -> i64 {
        Connection::open(path)
            .expect("open")
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version")
    };
    assert_eq!(version(&path), crate::records::PHOTO_SCHEMA_VERSION);
    let plain = mixed();
    let lossless: Vec<Bitmap> = plain.bitmaps().iter().map(Bitmap::as_lossless).collect();
    let plain = plain.with_bitmaps(lossless).expect("bitmaps");
    save(&plain, &path).expect("save");
    assert_eq!(version(&path), crate::records::BITMAP_SCHEMA_VERSION);
    assert!(
        crate::records::check_schema_version(crate::records::PHOTO_SCHEMA_VERSION + 1).is_err()
    );
}

#[test]
fn a_photo_record_that_lies_about_itself_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("t.flare");
    save(&mixed(), &path).expect("save");
    let photo = rows(&path)
        .into_iter()
        .find(|r| r.kind == "photo")
        .expect("photo");
    // Transparency after a photo that says it has none.
    let mut extra = photo.record.clone();
    extra.extend_from_slice(&[0, 1, 2]);
    put_record(&path, &photo.id, &extra);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    // Shorter than the JPEG it claims.
    put_record(&path, &photo.id, &photo.record[..photo.record.len() / 3]);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    // Not a record at all.
    put_record(&path, &photo.id, &[1, 0]);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}
