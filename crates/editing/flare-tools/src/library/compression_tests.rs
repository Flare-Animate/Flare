#![allow(clippy::expect_used)]

use super::*;
use flare_commands::apply_all;
use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{BitmapInstance, Content, FrameSpan, Layer, Scene, Timeline};
use std::num::NonZeroU32;

/// A smooth 96x64 picture, as a frame of video is.
fn frame(name: &str, level: u8) -> Bitmap {
    let rgba = (0..64u32)
        .flat_map(|y| (0..96u32).flat_map(move |x| [(x * 2) as u8, (y * 3) as u8, level, 255]))
        .collect();
    Bitmap::new(BitmapId::new(), name, 96, 64, rgba).expect("bitmap")
}

/// Two frames on the timeline and a dot (too small for a JPEG to help) in the library.
fn document() -> Document {
    let bitmaps = vec![
        frame("a", 10),
        frame("b", 200),
        Bitmap::new(BitmapId::new(), "dot", 1, 1, vec![1, 2, 3, 255]).expect("dot"),
    ];
    let spans = bitmaps
        .iter()
        .take(2)
        .enumerate()
        .map(|(i, b)| {
            let shown = BitmapInstance::new(InstanceId::new(), b.id(), Transform2D::IDENTITY);
            FrameSpan::new(
                i as u32,
                NonZeroU32::new(1).expect("nz"),
                Content::Bitmap(shown),
            )
        })
        .collect();
    let layer = Layer::new(LayerId::new(), "video", spans).expect("layer");
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
fn every_bitmap_becomes_a_photo_in_one_batch_and_the_frames_still_show_them() {
    let before = document();
    let done =
        set_bitmap_compression(&before, &[], Compression::Photo { quality: 85 }).expect("ok");
    assert_eq!((done.changed, done.unchanged, done.not_smaller), (2, 0, 1));
    assert!(
        done.bytes_after * 4 < done.bytes_before,
        "{} -> {}",
        done.bytes_before,
        done.bytes_after
    );
    let after = apply_all(&before, done.commands)
        .expect("valid")
        .expect("changed");
    let ids: Vec<BitmapId> = before.bitmaps().iter().map(Bitmap::id).collect();
    assert_eq!(
        ids,
        after.bitmaps().iter().map(Bitmap::id).collect::<Vec<_>>(),
        "same bitmaps, same order"
    );
    assert_eq!(after.bitmaps()[0].photo().map(|p| p.quality()), Some(85));
    assert!(
        after.bitmaps()[2].photo().is_none(),
        "the dot stays lossless"
    );
    // Asked again, nothing changes.
    let again =
        set_bitmap_compression(&after, &[], Compression::Photo { quality: 85 }).expect("ok");
    assert_eq!((again.changed, again.unchanged), (0, 2));
    assert!(again.commands.is_empty());
}

#[test]
fn only_the_named_bitmaps_change_and_lossless_takes_them_back() {
    let before = document();
    let first = before.bitmaps()[0].id();
    let done =
        set_bitmap_compression(&before, &[first], Compression::Photo { quality: 70 }).expect("ok");
    assert_eq!(done.changed, 1);
    let after = apply_all(&before, done.commands)
        .expect("valid")
        .expect("changed");
    assert!(
        after.bitmaps()[1].photo().is_none(),
        "the other frame is untouched"
    );
    let back = set_bitmap_compression(&after, &[first], Compression::Lossless).expect("ok");
    assert_eq!(back.changed, 1);
    let lossless = apply_all(&after, back.commands)
        .expect("valid")
        .expect("changed");
    assert!(lossless.bitmaps()[0].photo().is_none());
}

#[test]
fn a_missing_bitmap_an_empty_library_or_a_bad_quality_is_refused() {
    let before = document();
    let stranger = BitmapId::new();
    assert_eq!(
        set_bitmap_compression(&before, &[stranger], Compression::Lossless),
        Err(CompressionError::NoSuchBitmap(stranger))
    );
    for quality in [0, 101] {
        assert_eq!(
            set_bitmap_compression(&before, &[], Compression::Photo { quality }),
            Err(CompressionError::Quality(quality))
        );
    }
    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![]))],
        vec![],
    )
    .expect("document");
    assert_eq!(
        set_bitmap_compression(&empty, &[], Compression::Lossless),
        Err(CompressionError::NoBitmaps)
    );
}

#[test]
fn a_replacement_of_another_size_or_for_no_bitmap_is_refused_by_the_command() {
    let before = document();
    let mut wrong = frame("a", 10);
    assert!(
        apply_all(&before, [Command::ReplaceBitmap(wrong.clone())]).is_err(),
        "no such bitmap"
    );
    let small =
        Bitmap::new(before.bitmaps()[0].id(), "a", 1, 1, vec![0, 0, 0, 255]).expect("small");
    assert!(
        apply_all(&before, [Command::ReplaceBitmap(small)]).is_err(),
        "another size"
    );
    wrong = before.bitmaps()[0].as_photo(50).expect("photo");
    assert!(apply_all(&before, [Command::ReplaceBitmap(wrong)]).is_ok());
}
