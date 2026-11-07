//! The still formats the `image` crate writes beyond PNG, JPEG, BMP and TGA. Each takes the
//! straight-alpha pixels of a frame; a format with no alpha in it refuses a picture that has some.

use std::io::{Cursor, Write};

use ::image::{ExtendedColorType, ImageEncoder};

use crate::{Image, RenderError};

/// The largest side an ICO file can hold: its directory has one byte for the size.
pub const MAX_ICO_SIDE: u32 = 256;

/// Writes `image` as TIFF (lossless, with alpha). The encoder needs to seek, so it works in memory
/// first; a frame is bounded by the render limits.
pub(crate) fn write_tiff(image: &Image, mut writer: impl Write) -> Result<(), RenderError> {
    let mut buffer = Cursor::new(Vec::new());
    ::image::codecs::tiff::TiffEncoder::new(&mut buffer).write_image(
        &image.rgba,
        image.width,
        image.height,
        ExtendedColorType::Rgba8,
    )?;
    writer.write_all(buffer.get_ref())?;
    Ok(())
}

/// Writes `image` as lossless WebP, with alpha. The `image` crate has no lossy WebP encoder that
/// is pure Rust, so lossy WebP is not written.
pub(crate) fn write_webp(image: &Image, writer: impl Write) -> Result<(), RenderError> {
    ::image::codecs::webp::WebPEncoder::new_lossless(writer).write_image(
        &image.rgba,
        image.width,
        image.height,
        ExtendedColorType::Rgba8,
    )?;
    Ok(())
}

/// Writes `image` as QOI, with alpha.
pub(crate) fn write_qoi(image: &Image, writer: impl Write) -> Result<(), RenderError> {
    ::image::codecs::qoi::QoiEncoder::new(writer).write_image(
        &image.rgba,
        image.width,
        image.height,
        ExtendedColorType::Rgba8,
    )?;
    Ok(())
}

/// Writes `image` as a Netpbm arbitrary map (PAM, `P7`), which is the one Netpbm form that keeps
/// alpha. The `.pnm` name covers the whole family.
pub(crate) fn write_pnm(image: &Image, writer: impl Write) -> Result<(), RenderError> {
    ::image::codecs::pnm::PnmEncoder::new(writer)
        .with_subtype(::image::codecs::pnm::PnmSubtype::ArbitraryMap)
        .write_image(
            &image.rgba,
            image.width,
            image.height,
            ExtendedColorType::Rgba8,
        )?;
    Ok(())
}

/// Writes `image` as an ICO file holding one picture. ICO holds at most 256 pixels a side.
pub(crate) fn write_ico(image: &Image, writer: impl Write) -> Result<(), RenderError> {
    if image.width > MAX_ICO_SIDE || image.height > MAX_ICO_SIDE {
        return Err(RenderError::TooLargeFor {
            format: "ICO",
            side: MAX_ICO_SIDE,
        });
    }
    ::image::codecs::ico::IcoEncoder::new(writer).write_image(
        &image.rgba,
        image.width,
        image.height,
        ExtendedColorType::Rgba8,
    )?;
    Ok(())
}

/// Writes `image` as an OpenEXR file of 32-bit float channels, with alpha. Each 8-bit value becomes
/// its fraction of 255, so nothing is lost on the way back in.
pub(crate) fn write_exr(image: &Image, mut writer: impl Write) -> Result<(), RenderError> {
    let floats: Vec<u8> = image
        .rgba
        .iter()
        .flat_map(|&c| (f32::from(c) / 255.0).to_ne_bytes())
        .collect();
    let mut buffer = Cursor::new(Vec::new());
    ::image::codecs::openexr::OpenExrEncoder::new(&mut buffer).write_image(
        &floats,
        image.width,
        image.height,
        ExtendedColorType::Rgba32F,
    )?;
    writer.write_all(buffer.get_ref())?;
    Ok(())
}

/// Writes `image` as a Radiance HDR file. It has no alpha, so the picture must be opaque.
pub(crate) fn write_hdr(image: &Image, writer: impl Write) -> Result<(), RenderError> {
    let (pixels, _) = image.rgba.as_chunks::<4>();
    if pixels.iter().any(|px| px[3] != 255) {
        return Err(RenderError::NotOpaque);
    }
    let floats: Vec<u8> = pixels
        .iter()
        .flat_map(|px| [px[0], px[1], px[2]])
        .map(|c| f32::from(c) / 255.0)
        .flat_map(f32::to_ne_bytes)
        .collect();
    ::image::codecs::hdr::HdrEncoder::new(writer).write_image(
        &floats,
        image.width,
        image.height,
        ExtendedColorType::Rgb32F,
    )?;
    Ok(())
}
