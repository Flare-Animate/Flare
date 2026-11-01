//! The images a render produces: premultiplied for a texture, straight for a file.

use std::io::Write;

use flare_core::Rgba8;

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
        let opacity = opacity.clamp(0.0, 1.0);
        let mut out = self.clone();
        for px in bytemuck::cast_slice_mut::<u8, [u8; 4]>(&mut out.rgba) {
            px[3] = (f32::from(px[3]) * opacity).round() as u8;
        }
        out
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
