//! WebP: a still picture is read by the image crate's reader (see `bitmap::import_raster`); an
//! animated one is read frame by frame, each frame the whole canvas the decoder composed.

use std::io::Cursor;

use image_webp::WebPDecoder;

use crate::bitmap::{
    MAX_DECODED_BYTES, MAX_IMAGE_FRAMES, Picture, bad, check_size, import_animation,
};
use crate::xfl::{ImportError, Imported};

/// A WebP file as a project: an animation when it has frames to play, a still picture otherwise.
pub(crate) fn import(name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let animated = WebPDecoder::new(Cursor::new(bytes))
        .map(|decoder| decoder.is_animated())
        .unwrap_or(false);
    if animated {
        import_animation(name, decode_animation(name, bytes)?)
    } else {
        crate::bitmap::import_raster(name, bytes, image::ImageFormat::WebP)
    }
}

/// Every frame of an animated WebP, in order, as whole-canvas pictures with their times.
fn decode_animation(file: &str, bytes: &[u8]) -> Result<Vec<Picture>, ImportError> {
    let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(|e| bad(file, e))?;
    let (width, height) = decoder.dimensions();
    check_size(file, width, height)?;
    let frames = decoder.num_frames() as usize;
    if frames == 0 || frames > MAX_IMAGE_FRAMES {
        return Err(bad(
            file,
            format!("{frames} frames; an animation holds 1 to {MAX_IMAGE_FRAMES}"),
        ));
    }
    let canvas_bytes = u64::from(width) * u64::from(height) * 4;
    if canvas_bytes * frames as u64 > MAX_DECODED_BYTES {
        return Err(bad(file, "too many pixels across the frames"));
    }
    let has_alpha = decoder.has_alpha();
    let mut buffer = vec![
        0;
        decoder
            .output_buffer_size()
            .ok_or_else(|| bad(file, "too large"))?
    ];
    let mut pictures = Vec::with_capacity(frames);
    for _ in 0..frames {
        let millis = decoder.read_frame(&mut buffer).map_err(|e| bad(file, e))?;
        let rgba: Vec<u8> = if has_alpha {
            buffer.clone()
        } else {
            buffer
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect()
        };
        pictures.push(Picture {
            width,
            height,
            rgba,
            delay_cs: millis.div_ceil(10),
        });
    }
    Ok(pictures)
}
