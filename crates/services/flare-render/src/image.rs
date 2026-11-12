//! The images a render produces: premultiplied for a texture, straight for a file.

use std::io::Write;

use ::image::ImageEncoder;
use flare_core::Rgba8;

use crate::limits::{MAX_DIMENSION, MAX_PIXELS};

#[allow(unused_imports)]
use crate::RenderError;
#[allow(unused_imports)]
use crate::paint::{bez_path, clip_path, color, linear_gradient, stroke_style};

/// Premultiplied-alpha RGBA8 pixels, row-major from the top-left: what the rasterizer produces
/// and what a GPU or egui texture wants, so showing a frame needs no conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PremultipliedImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

impl PremultipliedImage {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// Straight-alpha RGBA8 pixels, row-major from the top-left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

impl Image {
    /// An image from straight-alpha RGBA8 pixels, row-major from the top-left. The size must be
    /// within the render limits and `rgba` must hold exactly `width * height` pixels.
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Image, RenderError> {
        let pixels = u64::from(width) * u64::from(height);
        let fits = (1..=MAX_DIMENSION).contains(&width)
            && (1..=MAX_DIMENSION).contains(&height)
            && pixels <= MAX_PIXELS;
        if !fits || rgba.len() as u64 != pixels * 4 {
            return Err(RenderError::InvalidSize(width, height));
        }
        Ok(Image {
            width,
            height,
            rgba,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgba8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = &self.rgba[i..i + 4];
        Some(Rgba8::new(p[0], p[1], p[2], p[3]))
    }

    /// A copy with every pixel's alpha multiplied by `opacity` (clamped to 0..=1). Colors are
    /// unchanged because the pixels are straight alpha.
    pub fn faded(&self, opacity: f32) -> Image {
        let mut out = self.clone();
        out.fade_in_place(opacity);
        out
    }

    /// [`Image::faded`] without a copy, for a ghost that is drawn only to be faded.
    pub(crate) fn fade_in_place(&mut self, opacity: f32) {
        let opacity = opacity.clamp(0.0, 1.0);
        for px in bytemuck::cast_slice_mut::<u8, [u8; 4]>(&mut self.rgba) {
            px[3] = (f32::from(px[3]) * opacity).round() as u8;
        }
    }

    /// A copy colored `tint` as onion-skin ghosts are when tinted: dark pixels (lines) become the
    /// tint and light ones (fills) a pale shade of it, so the drawing still reads; alpha is kept.
    pub fn tinted(&self, tint: Rgba8) -> Image {
        let mut out = self.clone();
        out.tint_in_place(tint);
        out
    }

    /// [`Image::tinted`] without a copy.
    pub(crate) fn tint_in_place(&mut self, tint: Rgba8) {
        let target = [tint.r, tint.g, tint.b].map(f32::from);
        for px in bytemuck::cast_slice_mut::<u8, [u8; 4]>(&mut self.rgba) {
            // Nothing drawn here: leave it, and save the arithmetic on the empty most of a ghost.
            if px[3] == 0 {
                continue;
            }
            let light =
                (0.2126 * f32::from(px[0]) + 0.7152 * f32::from(px[1]) + 0.0722 * f32::from(px[2]))
                    / 255.0;
            for (channel, to) in px.iter_mut().take(3).zip(target) {
                *channel = (to + (255.0 - to) * light * 0.7).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Draws `top` over this image (source-over, straight alpha). Sizes must match.
    pub fn composite(&mut self, top: &Image) -> Result<(), RenderError> {
        if (self.width, self.height) != (top.width, top.height) {
            return Err(RenderError::InvalidSize(top.width, top.height));
        }
        let pairs = self
            .rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(top.rgba.as_chunks::<4>().0);
        for (bottom, over) in pairs {
            let (ta, ba) = (u32::from(over[3]), u32::from(bottom[3]));
            if ta == 0 {
                continue;
            }
            // out_a = ta + ba (1 - ta), in 0..=255 fixed point with rounding.
            let out_a = ta * 255 + ba * (255 - ta);
            let out_a_scaled = ((out_a + 127) / 255).min(255);
            for channel in 0..3 {
                let top_part = u32::from(over[channel]) * ta * 255;
                let bottom_part = u32::from(bottom[channel]) * ba * (255 - ta);
                bottom[channel] =
                    ((top_part + bottom_part + out_a / 2) / out_a.max(1)).min(255) as u8;
            }
            bottom[3] = out_a_scaled as u8;
        }
        Ok(())
    }

    pub fn write_png(&self, writer: impl Write) -> Result<(), RenderError> {
        let mut encoder = png::Encoder::new(writer, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&self.rgba)?;
        Ok(())
    }

    /// Writes the image in `format`. The formats that keep transparency take the pixels as they
    /// are; a JPEG is refused unless every pixel is opaque, because flattening it silently would
    /// hide a background the person did not choose.
    pub fn write_still(
        &self,
        mut writer: impl Write,
        format: StillFormat,
    ) -> Result<(), RenderError> {
        match format {
            StillFormat::Png => self.write_png(writer),
            StillFormat::Jpeg { quality } => self.write_jpeg(writer, quality),
            StillFormat::Bmp => {
                ::image::codecs::bmp::BmpEncoder::new(&mut writer).write_image(
                    &self.rgba,
                    self.width,
                    self.height,
                    ::image::ExtendedColorType::Rgba8,
                )?;
                Ok(())
            }
            StillFormat::Tiff => crate::image_formats::write_tiff(self, writer),
            StillFormat::WebpLossless => crate::image_formats::write_webp(self, writer),
            StillFormat::Qoi => crate::image_formats::write_qoi(self, writer),
            StillFormat::Pnm => crate::image_formats::write_pnm(self, writer),
            StillFormat::Ico => crate::image_formats::write_ico(self, writer),
            StillFormat::Exr => crate::image_formats::write_exr(self, writer),
            StillFormat::Hdr => crate::image_formats::write_hdr(self, writer),
            StillFormat::Tga => {
                // Uncompressed on purpose: the encoder's RLE packets can run across a
                // scanline, which strict TGA readers (Pillow) refuse as a buffer overrun.
                ::image::codecs::tga::TgaEncoder::new(writer)
                    .disable_rle()
                    .write_image(
                        &self.rgba,
                        self.width,
                        self.height,
                        ::image::ExtendedColorType::Rgba8,
                    )?;
                Ok(())
            }
        }
    }

    /// Writes a baseline JPEG at `quality` (1 to 100). The alpha channel is dropped, so every
    /// pixel must already be opaque.
    fn write_jpeg(&self, writer: impl Write, quality: u8) -> Result<(), RenderError> {
        if !(1..=100).contains(&quality) {
            return Err(RenderError::InvalidQuality(quality));
        }
        let (pixels, _) = self.rgba.as_chunks::<4>();
        if pixels.iter().any(|px| px[3] != 255) {
            return Err(RenderError::NotOpaque);
        }
        let rgb: Vec<u8> = pixels.iter().flat_map(|px| [px[0], px[1], px[2]]).collect();
        ::image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality).write_image(
            &rgb,
            self.width,
            self.height,
            ::image::ExtendedColorType::Rgb8,
        )?;
        Ok(())
    }
}

/// A still-image format a frame can be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StillFormat {
    /// Lossless, keeps transparency.
    Png,
    /// Lossy and opaque. `quality` runs from 1 (smallest file) to 100 (best).
    Jpeg { quality: u8 },
    /// Uncompressed, keeps transparency.
    Bmp,
    /// Uncompressed, keeps transparency.
    Tga,
    /// Lossless, keeps transparency.
    Tiff,
    /// Lossless WebP, keeps transparency.
    WebpLossless,
    /// Lossless, keeps transparency.
    Qoi,
    /// Netpbm arbitrary map (PAM, `P7`), keeps transparency.
    Pnm,
    /// An ICO file of one picture; at most 256 pixels a side. Keeps transparency.
    Ico,
    /// OpenEXR with 32-bit float channels, keeps transparency.
    Exr,
    /// Radiance HDR. Opaque only.
    Hdr,
}

/// Converts premultiplied RGBA8 to straight alpha with rounding, in place. Opaque and fully
/// transparent pixels are already the same in both forms and are not touched.
pub(crate) fn unpremultiply_in_place(rgba: &mut [u8]) {
    for px in bytemuck::cast_slice_mut::<u8, [u8; 4]>(rgba) {
        let a = u32::from(px[3]);
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
        }
    }
}

#[cfg(test)]
#[path = "image_tint_tests.rs"]
mod tint_tests;
