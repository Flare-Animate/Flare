//! Animated WebP import. The container is built here from lossless frames the encoder writes, so
//! each frame's color and time are known exactly.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_import::import_foreign_bytes;

/// The lossless chunk (`VP8L`) of a 2 by 2 picture of one color.
fn lossless_frame(color: [u8; 4]) -> Vec<u8> {
    use image::ImageEncoder;
    let mut still = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut still)
        .write_image(&color.repeat(4), 2, 2, image::ExtendedColorType::Rgba8)
        .expect("webp");
    still[12..].to_vec()
}

/// A RIFF chunk: its name, its little-endian size, the payload, and a pad byte when odd.
fn chunk(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = name.to_vec();
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// A 2 by 2 animated WebP whose frames are the given (lossless chunk, duration in ms) pairs.
fn animated(frames: &[(Vec<u8>, u32)]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    // Animation flag and alpha flag; canvas width and height minus one.
    body.extend(chunk(b"VP8X", &[0x12, 0, 0, 0, 1, 0, 0, 1, 0, 0]));
    body.extend(chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]));
    for (lossless, millis) in frames {
        // Frame x and y halved, width and height minus one, then the duration (24 bits) and flags.
        let mut frame = vec![0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0];
        frame.extend(&millis.to_le_bytes()[..3]);
        // Bit 1 set: the frame replaces the canvas instead of being blended onto it, so its colors
        // come back exactly.
        frame.push(0x02);
        frame.extend(lossless);
        body.extend(chunk(b"ANMF", &frame));
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

#[test]
fn each_frame_is_a_picture_and_the_times_become_holds() {
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    let blue = [0, 0, 255, 255];
    let bytes = animated(&[
        (lossless_frame(red), 100),
        (lossless_frame(green), 200),
        (lossless_frame(blue), 100),
    ]);
    let imported = import_foreign_bytes(Path::new("a.webp"), bytes).expect("import");
    let doc = &imported.document;
    // 100, 200 and 100 ms: a 10 frame a second rate, holding each frame 1, 2 and 1 times.
    assert_eq!(doc.scenes()[0].timeline().frame_count(), 4);
    let pictures: Vec<Vec<u8>> = doc.bitmaps().iter().map(|b| b.pixels().to_vec()).collect();
    for color in [red, green, blue] {
        assert!(
            pictures
                .iter()
                .any(|p| p.as_chunks::<4>().0.iter().all(|px| *px == color)),
            "a picture of {color:?}"
        );
    }
}

#[test]
fn a_still_webp_stays_one_frame() {
    let mut still = Vec::new();
    use image::ImageEncoder;
    image::codecs::webp::WebPEncoder::new_lossless(&mut still)
        .write_image(
            &[9, 8, 7, 255].repeat(4),
            2,
            2,
            image::ExtendedColorType::Rgba8,
        )
        .expect("webp");
    let imported = import_foreign_bytes(Path::new("a.webp"), still).expect("import");
    assert_eq!(imported.document.scenes()[0].timeline().frame_count(), 1);
}

#[test]
fn a_damaged_animated_webp_is_refused_with_its_name() {
    let mut bytes = animated(&[(lossless_frame([1, 2, 3, 255]), 40)]);
    bytes.truncate(bytes.len() - 6);
    let error = import_foreign_bytes(Path::new("a.webp"), bytes).expect_err("refused");
    assert!(error.to_string().contains("a.webp"), "{error}");
}
