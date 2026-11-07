#![allow(clippy::expect_used)]

use std::path::Path;

use flare_document::Content;
use flare_import::{import_foreign_bytes, import_image_sequence};

fn png_bytes(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("header")
        .write_image_data(rgba)
        .expect("data");
    out
}

fn gif_bytes(delays: &[u16]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut out, 2, 2, &[]).expect("encoder");
        for (i, delay) in delays.iter().enumerate() {
            let shade = (i as u8 + 1) * 40;
            let mut pixels = [shade, 0, 0, 255].repeat(4);
            let mut frame = gif::Frame::from_rgba_speed(2, 2, &mut pixels, 10);
            frame.delay = *delay;
            encoder.write_frame(&frame).expect("frame");
        }
    }
    out
}

fn spans(imported: &flare_import::Imported) -> Vec<(u32, u32)> {
    imported.document.scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect()
}

#[test]
fn a_png_becomes_a_one_frame_project_with_the_pixels() {
    let pixels = [10, 20, 30, 255, 40, 50, 60, 128];
    let imported =
        import_foreign_bytes(Path::new("a.png"), png_bytes(2, 1, &pixels)).expect("import");
    let doc = &imported.document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (2, 1));
    assert_eq!(doc.bitmaps()[0].pixels(), pixels);
    assert_eq!(spans(&imported), vec![(0, 1)]);
}

#[test]
fn damaged_and_oversized_pngs_are_refused() {
    assert!(import_foreign_bytes(Path::new("a.png"), vec![1, 2, 3]).is_err());
    let good = png_bytes(2, 1, &[0; 8]);
    assert!(import_foreign_bytes(Path::new("a.png"), good[..good.len() - 12].to_vec()).is_err());
    // A header claiming 100000 x 100000 pixels is refused before any buffer is made.
    let mut huge = png_bytes(1, 1, &[0; 4]);
    huge[16..20].copy_from_slice(&100_000_u32.to_be_bytes());
    huge[20..24].copy_from_slice(&100_000_u32.to_be_bytes());
    assert!(import_foreign_bytes(Path::new("a.png"), huge).is_err());
}

#[test]
fn a_sequence_is_one_layer_in_natural_order_and_repeats_share_a_bitmap() {
    let a = png_bytes(1, 1, &[1, 1, 1, 255]);
    let b = png_bytes(1, 1, &[2, 2, 2, 255]);
    let files = vec![
        ("f_10.png".to_owned(), a.clone()),
        ("f_2.png".to_owned(), b),
        ("f_1.png".to_owned(), a),
    ];
    let imported = import_image_sequence(files).expect("sequence");
    let doc = &imported.document;
    assert_eq!(spans(&imported), vec![(0, 1), (1, 1), (2, 1)]);
    assert_eq!(doc.bitmaps().len(), 2, "f_1 and f_10 are the same picture");
    let layer = &doc.scenes()[0].timeline().layers()[0];
    let shown: Vec<u8> = layer
        .spans()
        .iter()
        .map(|s| match s.content() {
            Content::Bitmap(b) => doc.bitmap(b.bitmap()).expect("bitmap").pixels()[0],
            _ => 0,
        })
        .collect();
    assert_eq!(shown, vec![1, 2, 1]);
    assert!(import_image_sequence(vec![]).is_err());
}

#[test]
fn a_gif_delays_become_the_frame_rate_and_hold_counts() {
    let imported =
        import_foreign_bytes(Path::new("a.gif"), gif_bytes(&[10, 20, 10])).expect("import");
    assert_eq!(
        imported.document.frame_rate(),
        flare_core::FrameRate::new(10, 1).expect("rate")
    );
    assert_eq!(spans(&imported), vec![(0, 1), (1, 2), (3, 1)]);
    assert!(import_foreign_bytes(Path::new("a.gif"), b"GIF89a".to_vec()).is_err());
}

#[test]
fn a_jpeg_becomes_a_one_frame_project_of_the_picture_size() {
    let bytes = include_bytes!("fixtures/tiny-4x4.jpg").to_vec();
    for name in ["a.jpg", "a.JPEG"] {
        let imported = import_foreign_bytes(Path::new(name), bytes.clone()).expect("import");
        let doc = &imported.document;
        assert_eq!(
            (doc.stage().width(), doc.stage().height()),
            (4, 4),
            "{name}"
        );
        assert_eq!(spans(&imported), vec![(0, 1)], "{name}");
    }
}

#[test]
fn a_damaged_jpeg_is_refused_with_its_name() {
    let error = import_foreign_bytes(Path::new("a.jpg"), b"\xFF\xD8\xFF not a jpeg".to_vec())
        .expect_err("refused");
    assert!(error.to_string().contains("a.jpg"), "{error}");
    let good = include_bytes!("fixtures/tiny-4x4.jpg");
    assert!(import_foreign_bytes(Path::new("a.jpg"), good[..good.len() / 2].to_vec()).is_err());
}

/// A 3 by 2 picture with varied colors and alpha, as the exporter writes it.
fn raster_pixels() -> Vec<u8> {
    (0..6u8)
        .flat_map(|i| [i * 40, 255 - i * 30, i * 7, 60 + i * 30])
        .collect()
}

#[test]
fn a_bmp_or_tga_export_imports_back_to_the_same_pixels() {
    use image::{
        ExtendedColorType, ImageEncoder, codecs::bmp::BmpEncoder, codecs::tga::TgaEncoder,
    };
    let pixels = raster_pixels();
    let mut bmp = Vec::new();
    BmpEncoder::new(&mut bmp)
        .write_image(&pixels, 3, 2, ExtendedColorType::Rgba8)
        .expect("bmp");
    let mut tga = Vec::new();
    TgaEncoder::new(&mut tga)
        .disable_rle()
        .write_image(&pixels, 3, 2, ExtendedColorType::Rgba8)
        .expect("tga");
    for (name, bytes) in [("a.bmp", bmp), ("a.tga", tga)] {
        let imported = import_foreign_bytes(Path::new(name), bytes).expect(name);
        let doc = &imported.document;
        assert_eq!(
            (doc.stage().width(), doc.stage().height()),
            (3, 2),
            "{name}"
        );
        assert_eq!(doc.bitmaps()[0].pixels(), pixels, "{name}");
        assert_eq!(spans(&imported), vec![(0, 1)], "{name}");
    }
}

#[test]
fn a_damaged_or_truncated_bmp_or_tga_is_refused_with_its_name() {
    for name in ["a.bmp", "a.tga"] {
        let error = import_foreign_bytes(Path::new(name), b"not a picture at all".to_vec())
            .expect_err(name);
        assert!(error.to_string().contains(name), "{name}: {error}");
    }
    let bmp = {
        use image::ImageEncoder;
        let mut out = Vec::new();
        image::codecs::bmp::BmpEncoder::new(&mut out)
            .write_image(&raster_pixels(), 3, 2, image::ExtendedColorType::Rgba8)
            .expect("bmp");
        out
    };
    assert!(import_foreign_bytes(Path::new("a.bmp"), bmp[..bmp.len() / 2].to_vec()).is_err());
}
