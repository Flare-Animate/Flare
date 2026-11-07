//! Still-image encoders: each format round-trips through its own decoder, and the refusals
//! (quality out of range, transparency in a JPEG, a failing writer) leave nothing behind.
#![allow(clippy::expect_used)]

use crate::{Image, RenderError, StillFormat};
use std::io::{self, Write};

/// A 2x2 image with four different straight-alpha pixels.
fn mixed_alpha() -> Image {
    Image {
        width: 2,
        height: 2,
        rgba: vec![
            255, 0, 0, 255, // red, opaque
            0, 255, 0, 128, // green, half transparent
            0, 0, 255, 0, // blue, fully transparent
            10, 20, 30, 255, // dark, opaque
        ],
    }
}

fn opaque_flat(width: u32, height: u32, rgb: [u8; 3]) -> Image {
    let pixels = (width * height) as usize;
    let rgba = (0..pixels)
        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
        .collect();
    Image {
        width,
        height,
        rgba,
    }
}

fn encode(image: &Image, format: StillFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image.write_still(&mut bytes, format).expect("encode");
    bytes
}

/// A writer that refuses every byte, to show a write error is reported as an error.
struct Refuses;

impl Write for Refuses {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("disk full"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn png_round_trips_pixels_and_alpha_exactly() {
    let image = mixed_alpha();
    let bytes = encode(&image, StillFormat::Png);
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes.as_slice()))
        .read_info()
        .expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    assert_eq!((info.width, info.height), (2, 2));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(&buf[..info.buffer_size()], image.rgba.as_slice());
}

#[test]
fn tga_keeps_transparency_and_pixels() {
    let image = mixed_alpha();
    let decoded = ::image::load_from_memory_with_format(
        &encode(&image, StillFormat::Tga),
        ::image::ImageFormat::Tga,
    )
    .expect("decode tga")
    .to_rgba8();
    assert_eq!(decoded.dimensions(), (2, 2));
    assert_eq!(decoded.as_raw(), &image.rgba);
}

#[test]
fn tga_is_uncompressed_truecolour_so_strict_readers_accept_it() {
    // Type 2 is uncompressed truecolour. The RLE type (10) can put a packet across two
    // scanlines, which Pillow reads as a buffer overrun.
    let image = mixed_alpha();
    let bytes = encode(&image, StillFormat::Tga);
    assert_eq!(bytes[2], 2, "image type");
    assert_eq!(bytes[16], 32, "bits per pixel");
    assert!(bytes.len() >= 18 + 2 * 2 * 4, "pixel data is all there");
}

#[test]
fn bmp_writes_the_opaque_colours_of_the_frame() {
    let image = opaque_flat(3, 2, [200, 100, 50]);
    let decoded = ::image::load_from_memory_with_format(
        &encode(&image, StillFormat::Bmp),
        ::image::ImageFormat::Bmp,
    )
    .expect("decode bmp")
    .to_rgb8();
    assert_eq!(decoded.dimensions(), (3, 2));
    assert!(decoded.pixels().all(|p| p.0 == [200, 100, 50]));
}

#[test]
fn jpeg_is_an_opaque_jpeg_at_the_requested_size() {
    let image = opaque_flat(16, 16, [40, 120, 220]);
    let bytes = encode(&image, StillFormat::Jpeg { quality: 100 });
    assert_eq!(
        &bytes[..3],
        &[0xFF, 0xD8, 0xFF],
        "JPEG start-of-image marker"
    );
    let decoded = ::image::load_from_memory_with_format(&bytes, ::image::ImageFormat::Jpeg)
        .expect("decode jpeg")
        .to_rgb8();
    assert_eq!(decoded.dimensions(), (16, 16));
    // JPEG is lossy; a flat colour survives within a few levels.
    for pixel in decoded.pixels() {
        for (got, want) in pixel.0.iter().zip([40u8, 120, 220]) {
            assert!(got.abs_diff(want) <= 6, "{:?} vs {want}", pixel.0);
        }
    }
}

#[test]
fn jpeg_quality_must_be_between_one_and_one_hundred() {
    let image = opaque_flat(4, 4, [0, 0, 0]);
    for quality in [0u8, 101, 255] {
        let mut bytes = Vec::new();
        let error = image
            .write_still(&mut bytes, StillFormat::Jpeg { quality })
            .expect_err("quality outside 1 to 100");
        assert!(matches!(error, RenderError::InvalidQuality(q) if q == quality));
        assert!(bytes.is_empty(), "a refused quality writes nothing");
    }
    for quality in [1u8, 100] {
        encode(&image, StillFormat::Jpeg { quality });
    }
}

#[test]
fn jpeg_refuses_a_transparent_pixel_and_writes_nothing() {
    let mut bytes = Vec::new();
    let error = mixed_alpha()
        .write_still(&mut bytes, StillFormat::Jpeg { quality: 90 })
        .expect_err("alpha below 255");
    assert!(matches!(error, RenderError::NotOpaque));
    assert!(bytes.is_empty());
}

#[test]
fn a_failing_writer_is_reported_for_every_format() {
    let image = opaque_flat(2, 2, [1, 2, 3]);
    for format in [
        StillFormat::Png,
        StillFormat::Jpeg { quality: 80 },
        StillFormat::Bmp,
        StillFormat::Tga,
        StillFormat::Tiff,
        StillFormat::WebpLossless,
        StillFormat::Qoi,
        StillFormat::Pnm,
        StillFormat::Ico,
        StillFormat::Exr,
        StillFormat::Hdr,
    ] {
        let result = image.write_still(Refuses, format);
        assert!(result.is_err(), "{format:?} must report the failed write");
    }
}

/// Decodes `bytes` with the `image` crate's own reader for `format`, as 8-bit RGBA.
fn decode_rgba(bytes: &[u8], format: ::image::ImageFormat) -> ::image::RgbaImage {
    ::image::load_from_memory_with_format(bytes, format)
        .expect("decode")
        .to_rgba8()
}

#[test]
fn lossless_formats_keep_every_pixel_and_its_alpha() {
    let image = mixed_alpha();
    let cases = [
        (StillFormat::Tiff, ::image::ImageFormat::Tiff),
        (StillFormat::WebpLossless, ::image::ImageFormat::WebP),
        (StillFormat::Qoi, ::image::ImageFormat::Qoi),
        (StillFormat::Pnm, ::image::ImageFormat::Pnm),
        (StillFormat::Ico, ::image::ImageFormat::Ico),
        (StillFormat::Exr, ::image::ImageFormat::OpenExr),
    ];
    for (format, decoder) in cases {
        let decoded = decode_rgba(&encode(&image, format), decoder);
        assert_eq!(decoded.dimensions(), (2, 2), "{format:?}");
        // EXR goes through float, which may round a channel by one level.
        for (got, want) in decoded.as_raw().iter().zip(&image.rgba) {
            assert!(got.abs_diff(*want) <= 1, "{format:?}: {got} vs {want}");
        }
    }
}

#[test]
fn hdr_is_an_opaque_picture_within_its_shared_exponent_rounding() {
    let image = opaque_flat(3, 2, [200, 100, 7]);
    let decoded = ::image::load_from_memory_with_format(
        &encode(&image, StillFormat::Hdr),
        ::image::ImageFormat::Hdr,
    )
    .expect("decode hdr")
    .to_rgb32f();
    assert_eq!(decoded.dimensions(), (3, 2));
    for pixel in decoded.pixels() {
        for (got, want) in pixel.0.iter().zip([200u8, 100, 7]) {
            let level = (got * 255.0).round() as i32;
            assert!((level - i32::from(want)).abs() <= 2, "{level} vs {want}");
        }
    }
}

#[test]
fn hdr_refuses_transparency_and_writes_nothing() {
    let mut bytes = Vec::new();
    let error = mixed_alpha()
        .write_still(&mut bytes, StillFormat::Hdr)
        .expect_err("alpha below 255");
    assert!(matches!(error, RenderError::NotOpaque));
    assert!(bytes.is_empty());
}

#[test]
fn ico_refuses_a_picture_past_256_pixels_and_writes_nothing() {
    let image = opaque_flat(257, 1, [0, 0, 0]);
    let mut bytes = Vec::new();
    let error = image
        .write_still(&mut bytes, StillFormat::Ico)
        .expect_err("too wide for ICO");
    assert!(matches!(error, RenderError::TooLargeFor { side: 256, .. }));
    assert!(bytes.is_empty());
    let fits = opaque_flat(256, 1, [0, 0, 0]);
    encode(&fits, StillFormat::Ico);
}
