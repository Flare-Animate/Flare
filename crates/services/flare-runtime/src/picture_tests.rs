//! Picture files by their ending: the bytes each name gets, and the refusals.
#![allow(clippy::expect_used)]

use super::*;
use flare_core::{Rgba8, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use std::num::NonZeroU32;

fn a_red_square() -> Document {
    let shape = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        10.0,
        10.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(3).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(flare_core::LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(flare_core::SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        flare_core::DocumentId::new(),
        flare_core::FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn a_still_ending_gives_that_format_whatever_its_case() {
    let document = a_red_square();
    let png = picture_bytes(&document, 0, "PNG", Backdrop::Stage).expect("png");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let jpeg = picture_bytes(&document, 0, "jpg", Backdrop::Stage).expect("jpeg");
    assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
    let hdr = picture_bytes(&document, 0, "hdr", Backdrop::Stage).expect("hdr");
    assert!(hdr.starts_with(b"#?RADIANCE") || hdr.starts_with(b"#?RGBE"));
}

#[test]
fn psd_is_a_still_picture_of_the_frame_too() {
    let document = a_red_square();
    assert!(is_still_ending("PSD"));
    let psd = picture_bytes(&document, 0, "psd", Backdrop::Stage).expect("psd");
    assert_eq!(&psd[..4], b"8BPS");
}

#[test]
fn every_still_ending_is_written_by_picture_bytes() {
    let document = a_red_square();
    for ending in STILL_ENDINGS {
        // An ICO holds at most 256 pixels a side, and this stage is larger: that refusal is the
        // only one an ending in the list may get here.
        if let Err(error) = picture_bytes(&document, 0, ending, Backdrop::Stage) {
            assert!(
                ending == "ico" && error.contains("ICO holds at most 256 pixels a side"),
                "{ending} is listed but not written: {error}"
            );
        }
    }
}

#[test]
fn an_animation_ending_gives_a_gif_or_an_apng() {
    let document = a_red_square();
    let gif = picture_bytes(&document, 0, "gif", Backdrop::Stage).expect("gif");
    assert!(gif.starts_with(b"GIF89a") || gif.starts_with(b"GIF87a"));
    let apng = picture_bytes(&document, 0, "apng", Backdrop::Stage).expect("apng");
    assert_eq!(&apng[..8], b"\x89PNG\r\n\x1a\n");
    assert!(
        apng.windows(4).any(|w| w == b"acTL"),
        "an APNG has an animation control chunk"
    );
}

#[test]
fn a_format_with_no_alpha_is_refused_over_a_transparent_backdrop() {
    let document = a_red_square();
    let error = picture_bytes(&document, 0, "jpg", Backdrop::Transparent)
        .expect_err("a JPEG has no alpha channel");
    assert!(
        error.contains("JPEG"),
        "the refusal names the format: {error}"
    );
    assert!(picture_bytes(&document, 0, "hdr", Backdrop::Transparent).is_err());
}

#[test]
fn an_ending_that_is_no_picture_is_refused_by_name_and_movies_are_left_to_the_desktop() {
    let document = a_red_square();
    for ending in ["svg", "mp4", "mov", "webm", ""] {
        let error =
            picture_bytes(&document, 0, ending, Backdrop::Stage).expect_err("not a picture format");
        assert!(
            error.contains("A picture is written"),
            "{ending:?}: {error}"
        );
    }
}
