//! A bitmap kept as a photo, as Animate's "Photo (JPEG)" compression keeps one: its colors as a
//! JPEG, and its transparency, when it has any, exactly beside it. The pixels the document holds
//! are the ones the JPEG decodes to, so what is drawn is what is saved, and a photo saved and
//! opened again is the same bitmap.
//!
//! A frame of video is the case it is for: the video was compressed already, and as a JPEG a
//! frame takes about a ninth of the room its pixels do. A drawing or a screenshot, whose hard
//! edges JPEG blurs, stays lossless unless someone chooses otherwise.

use std::io::Cursor;
use std::sync::Arc;

use flare_core::BitmapId;
use image::codecs::jpeg::{JpegDecoder, JpegEncoder};
use image::{ColorType, ExtendedColorType, ImageDecoder, ImageEncoder, Limits};

use crate::{Bitmap, BitmapError};

/// The quality a photo gets when none is chosen: measured on video frames, where it cannot be
/// told from the frame it came from and still takes about a ninth of the room.
pub const DEFAULT_PHOTO_QUALITY: u8 = 92;

/// How a bitmap kept as a photo is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Photo {
    quality: u8,
    jpeg: Arc<[u8]>,
    /// One byte a pixel, rows top to bottom; `None` when every pixel is opaque.
    alpha: Option<Arc<[u8]>>,
}

impl Photo {
    /// The JPEG quality it was made at, 1 to 100.
    pub fn quality(&self) -> u8 {
        self.quality
    }

    /// The JPEG of its colors.
    pub fn jpeg(&self) -> &[u8] {
        &self.jpeg
    }

    /// Its transparency, a byte a pixel, if any pixel is not opaque.
    pub fn alpha(&self) -> Option<&[u8]> {
        self.alpha.as_deref()
    }

    /// The bytes it takes to store.
    pub fn byte_len(&self) -> usize {
        self.jpeg.len() + self.alpha.as_ref().map_or(0, |a| a.len())
    }
}

fn quality_ok(quality: u8) -> Result<u8, BitmapError> {
    if (1..=100).contains(&quality) {
        Ok(quality)
    } else {
        Err(BitmapError::PhotoQuality(quality))
    }
}

impl Bitmap {
    /// How the bitmap is stored, when it is kept as a photo.
    pub fn photo(&self) -> Option<&Photo> {
        self.photo.as_ref()
    }

    /// The same bitmap (same identity and name) kept as a photo at `quality`, its pixels now the
    /// ones the JPEG decodes to. A photo already at that quality is returned as it is; one at
    /// another is compressed again from the pixels it has.
    ///
    /// # Errors
    /// A quality outside 1 to 100, a JPEG that would take as much room as the pixels
    /// ([`BitmapError::PhotoNotSmaller`]), or one the encoder could not make.
    pub fn as_photo(&self, quality: u8) -> Result<Bitmap, BitmapError> {
        let quality = quality_ok(quality)?;
        if self.photo.as_ref().is_some_and(|p| p.quality == quality) {
            return Ok(self.clone());
        }
        let rgb: Vec<u8> = self
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality)
            .write_image(&rgb, self.width(), self.height(), ExtendedColorType::Rgb8)
            .map_err(|e| BitmapError::Photo(e.to_string()))?;
        let alpha = self
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] != 255)
            .then(|| {
                self.pixels()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| p[3])
                    .collect::<Vec<_>>()
            });
        if jpeg.len() + alpha.as_ref().map_or(0, Vec::len) >= self.pixels().len() {
            return Err(BitmapError::PhotoNotSmaller);
        }
        Bitmap::from_photo(
            self.id(),
            self.name(),
            self.width(),
            self.height(),
            quality,
            jpeg,
            alpha,
        )
    }

    /// A bitmap from a stored photo: the JPEG is decoded, and must be exactly `width` by
    /// `height`, and `alpha`, when given, holds a byte for every pixel. The JPEG is untrusted:
    /// the decoder may not make a picture larger than the one declared.
    ///
    /// # Errors
    /// A size or quality out of range, a JPEG that is not one or is another size, or an alpha of
    /// the wrong length.
    pub fn from_photo(
        id: BitmapId,
        name: impl Into<String>,
        width: u32,
        height: u32,
        quality: u8,
        jpeg: Vec<u8>,
        alpha: Option<Vec<u8>>,
    ) -> Result<Bitmap, BitmapError> {
        let quality = quality_ok(quality)?;
        // The declared size is checked (side and pixel count) before the decoder is asked.
        Bitmap::check_size(width, height)?;
        let pixels = width as usize * height as usize;
        if alpha.as_ref().is_some_and(|a| a.len() != pixels) {
            return Err(BitmapError::Photo(
                "its transparency is not one byte a pixel".into(),
            ));
        }
        let colors = decode(&jpeg, width, height)?;
        let mut rgba = Vec::with_capacity(pixels * 4);
        for (i, rgb) in colors.as_chunks::<3>().0.iter().enumerate() {
            let a = alpha.as_ref().map_or(255, |a| a[i]);
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
        }
        let mut bitmap = Bitmap::new(id, name, width, height, rgba)?;
        bitmap.photo = Some(Photo {
            quality,
            jpeg: jpeg.into(),
            alpha: alpha.map(Into::into),
        });
        Ok(bitmap)
    }

    /// The same bitmap stored losslessly from now on, its pixels as they are.
    pub fn as_lossless(&self) -> Bitmap {
        let mut bitmap = self.clone();
        bitmap.photo = None;
        bitmap
    }
}

/// The colors of `jpeg`, three bytes a pixel, which must be `width` by `height`.
fn decode(jpeg: &[u8], width: u32, height: u32) -> Result<Vec<u8>, BitmapError> {
    let bad = |e: image::ImageError| BitmapError::Photo(e.to_string());
    let mut decoder = JpegDecoder::new(Cursor::new(jpeg)).map_err(bad)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(u64::from(width) * u64::from(height) * 4 + 1024 * 1024);
    decoder.set_limits(limits).map_err(bad)?;
    if decoder.dimensions() != (width, height) {
        return Err(BitmapError::Photo(format!(
            "the JPEG is {}x{}, not {width}x{height}",
            decoder.dimensions().0,
            decoder.dimensions().1
        )));
    }
    let color = decoder.color_type();
    let mut out = vec![0; usize::try_from(decoder.total_bytes()).unwrap_or(usize::MAX)];
    decoder.read_image(&mut out).map_err(bad)?;
    match color {
        ColorType::Rgb8 => Ok(out),
        // A gray JPEG (another program's) is the same picture in color.
        ColorType::L8 => Ok(out.iter().flat_map(|&v| [v, v, v]).collect()),
        other => Err(BitmapError::Photo(format!(
            "a JPEG of {other:?} is not read"
        ))),
    }
}

#[cfg(test)]
#[path = "bitmap_photo_tests.rs"]
mod tests;
