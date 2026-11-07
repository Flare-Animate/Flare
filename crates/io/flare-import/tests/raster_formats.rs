//! Pictures written by flare-render's still encoders, read back by the importer: each format's
//! pixels survive the trip, and a damaged file is refused with its name.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_import::import_foreign_bytes;
use flare_render::{Image, StillFormat};

/// A 3 by 2 picture with varied colors and alpha.
fn mixed() -> Image {
    let rgba = (0..6u8)
        .flat_map(|i| [i * 40, 255 - i * 30, i * 7, 60 + i * 30])
        .collect();
    Image::from_rgba(3, 2, rgba).expect("picture")
}

fn opaque() -> Image {
    let rgba = (0..6u8)
        .flat_map(|i| [i * 40, 255 - i * 30, i * 7, 255])
        .collect();
    Image::from_rgba(3, 2, rgba).expect("picture")
}

fn written(image: &Image, format: StillFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image.write_still(&mut bytes, format).expect("write");
    bytes
}

/// The imported picture's size and pixels.
fn read_back(name: &str, bytes: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let imported = import_foreign_bytes(Path::new(name), bytes).expect(name);
    let doc = &imported.document;
    let pixels = doc.bitmaps()[0].pixels().to_vec();
    (doc.stage().width(), doc.stage().height(), pixels)
}

#[test]
fn lossless_formats_come_back_with_every_pixel_and_its_alpha() {
    let image = mixed();
    for (format, name) in [
        (StillFormat::Tiff, "a.tif"),
        (StillFormat::Tiff, "a.tiff"),
        (StillFormat::WebpLossless, "a.webp"),
        (StillFormat::Qoi, "a.qoi"),
        (StillFormat::Pnm, "a.pnm"),
        (StillFormat::Ico, "a.ico"),
    ] {
        let (w, h, pixels) = read_back(name, written(&image, format));
        assert_eq!((w, h), (3, 2), "{name}");
        assert_eq!(pixels, image.rgba(), "{name}");
    }
}

#[test]
fn exr_comes_back_within_one_level_of_each_channel() {
    let image = mixed();
    let (w, h, pixels) = read_back("a.exr", written(&image, StillFormat::Exr));
    assert_eq!((w, h), (3, 2));
    for (got, want) in pixels.iter().zip(image.rgba()) {
        assert!(got.abs_diff(*want) <= 1, "{got} vs {want}");
    }
}

#[test]
fn hdr_comes_back_opaque_within_its_shared_exponent_rounding() {
    let image = opaque();
    let (_, _, pixels) = read_back("a.hdr", written(&image, StillFormat::Hdr));
    for (px, want) in pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(image.rgba().as_chunks::<4>().0)
    {
        assert_eq!(px[3], 255);
        for (got, want) in px[..3].iter().zip(&want[..3]) {
            assert!(got.abs_diff(*want) <= 2, "{got} vs {want}");
        }
    }
}

#[test]
fn a_plain_netpbm_ppm_is_a_picture_and_not_a_flipnote() {
    let bytes = b"P3\n1 1\n255\n1 2 3\n".to_vec();
    let (w, h, pixels) = read_back("a.ppm", bytes);
    assert_eq!((w, h), (1, 1));
    assert_eq!(pixels, vec![1, 2, 3, 255]);
}

#[test]
fn damaged_files_of_each_new_format_are_refused_with_their_name() {
    for name in [
        "a.tif", "a.webp", "a.ico", "a.qoi", "a.pnm", "a.hdr", "a.exr", "a.dds",
    ] {
        let error =
            import_foreign_bytes(Path::new(name), b"junk, not a picture".to_vec()).expect_err(name);
        assert!(error.to_string().contains(name), "{name}: {error}");
    }
}

#[test]
fn a_truncated_tiff_is_refused() {
    let bytes = written(&mixed(), StillFormat::Tiff);
    assert!(import_foreign_bytes(Path::new("a.tif"), bytes[..bytes.len() / 2].to_vec()).is_err());
}
