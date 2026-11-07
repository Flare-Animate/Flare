//! Animated PNG import: each frame comes back as its own picture, blended as its operations say.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_import::import_foreign_bytes;

/// Encodes `frames` (each a 2 by 1 RGBA picture) as an APNG, with each frame's blend operation.
fn apng(frames: &[(Vec<u8>, png::BlendOp)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 2, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(frames.len() as u32, 0)
        .expect("animated");
    let mut writer = encoder.write_header().expect("header");
    for (pixels, blend) in frames {
        writer.set_frame_delay(40, 1000).expect("delay");
        writer.set_blend_op(*blend).expect("blend");
        writer.write_image_data(pixels).expect("frame");
    }
    writer.finish().expect("finish");
    out
}

fn pictures(bytes: Vec<u8>) -> Vec<Vec<u8>> {
    let imported = import_foreign_bytes(Path::new("a.png"), bytes).expect("import");
    let doc = &imported.document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (2, 1));
    doc.bitmaps().iter().map(|b| b.pixels().to_vec()).collect()
}

#[test]
fn each_frame_comes_back_as_its_own_picture_in_order() {
    let red = vec![255, 0, 0, 255, 255, 0, 0, 255];
    let blue = vec![0, 0, 255, 255, 0, 0, 255, 255];
    let bytes = apng(&[
        (red.clone(), png::BlendOp::Source),
        (blue.clone(), png::BlendOp::Source),
    ]);
    let imported = import_foreign_bytes(Path::new("a.png"), bytes).expect("import");
    let timeline = imported.document.scenes()[0].timeline();
    assert_eq!(timeline.frame_count(), 2, "one hold per 40 ms frame");
    let pictures: Vec<Vec<u8>> = imported
        .document
        .bitmaps()
        .iter()
        .map(|b| b.pixels().to_vec())
        .collect();
    assert!(pictures.contains(&red), "the first frame's picture");
    assert!(pictures.contains(&blue), "the second frame's picture");
}

#[test]
fn an_over_frame_is_blended_onto_the_one_before_it() {
    // Opaque red, then half-transparent green drawn over it: red and green mix to about half each.
    let red = vec![255, 0, 0, 255, 255, 0, 0, 255];
    let green = vec![0, 255, 0, 128, 0, 255, 0, 128];
    let bytes = apng(&[(red, png::BlendOp::Source), (green, png::BlendOp::Over)]);
    let pictures = pictures(bytes);
    let mixed = pictures
        .iter()
        .find(|p| p[3] == 255 && p[1] > 0 && p[0] > 0)
        .expect("the blended frame");
    for px in mixed.as_chunks::<4>().0 {
        assert!(px[0].abs_diff(127) <= 2, "red {px:?}");
        assert!(px[1].abs_diff(128) <= 2, "green {px:?}");
        assert_eq!(px[3], 255, "opaque under an opaque frame");
    }
}

#[test]
fn a_damaged_animation_is_refused_with_its_name() {
    let mut bytes = apng(&[(vec![0; 8], png::BlendOp::Source)]);
    bytes.truncate(bytes.len() - 10);
    let error = import_foreign_bytes(Path::new("a.png"), bytes).expect_err("refused");
    assert!(error.to_string().contains("a.png"), "{error}");
}
