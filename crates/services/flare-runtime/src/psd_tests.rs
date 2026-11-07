//! The flattened PSD export: the frame comes back out of the PSD as the same pixels the PNG export
//! draws, read by the importer and by the `psd` crate alike.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_core::{Rgba8, ShapeId};
use flare_document::Shape;

use crate::svg_tests::document;

/// The pixels of the PNG export of frame 0, decoded.
fn png_pixels(document: &flare_document::Document) -> (u32, u32, Vec<u8>) {
    let bytes = crate::png_bytes(document, 0).expect("png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .expect("info");
    let mut buffer = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buffer).expect("frame");
    (
        info.width,
        info.height,
        buffer[..info.buffer_size()].to_vec(),
    )
}

#[test]
fn a_flattened_psd_carries_the_frame_pixels_to_the_importer_and_the_psd_reader() {
    let red = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        10.0,
        8.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let doc = document(vec![red]);
    let (width, height, expected) = png_pixels(&doc);
    let bytes = crate::psd_export(&doc, 0).expect("psd");

    let parsed = psd::Psd::from_bytes(&bytes).expect("the psd crate reads it");
    assert_eq!((parsed.width(), parsed.height()), (width, height));
    assert_eq!(
        parsed.rgba(),
        expected,
        "the PSD reader sees the same pixels"
    );

    let imported = flare_import::import_foreign_bytes(Path::new("a.psd"), bytes).expect("import");
    assert_eq!(
        (
            imported.document.stage().width(),
            imported.document.stage().height()
        ),
        (width, height)
    );
    assert_eq!(imported.document.bitmaps()[0].pixels(), expected.as_slice());
}

#[test]
fn a_psd_export_of_a_stage_past_the_psd_limit_is_refused_in_words() {
    let error = flare_export::psd::flattened_psd(30_001, 1, &[0; 4]).expect_err("too wide");
    assert!(error.to_string().contains("30000"), "{error}");
    let short = flare_export::psd::flattened_psd(2, 2, &[0; 4]).expect_err("short picture");
    assert!(short.to_string().contains("bytes"), "{short}");
}
