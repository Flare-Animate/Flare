//! A still picture read only to be looked at, never put in a document: the reference a
//! mannequin is posed from. Any picture kind Flare imports is read (the first frame of an
//! animation), with the same limits, and made smaller when it is larger than it will be shown,
//! so a photograph from a camera does not fill the memory of the screen.

use crate::bitmap::{Picture, bad, decode_gif, decode_jpeg, decode_png, decode_raster};
use crate::xfl::ImportError;

/// The largest picture file read.
pub const MAX_STILL_BYTES: usize = 64 * 1024 * 1024;
/// The endings of the picture files read.
pub const STILL_EXTENSIONS: [&str; 18] = [
    "png", "jpg", "jpeg", "gif", "bmp", "tga", "tif", "tiff", "webp", "qoi", "pnm", "pbm", "pgm",
    "pam", "ico", "hdr", "exr", "dds",
];

/// A picture to look at: its size as the file has it, and its pixels (straight RGBA) at the
/// size it is kept at, no larger than was asked.
#[derive(Debug, Clone, PartialEq)]
pub struct Still {
    pub width: u32,
    pub height: u32,
    pub kept: [u32; 2],
    pub rgba: Vec<u8>,
}

/// Reads the picture in `bytes`, its kind told by `name`'s ending, keeping it no more than
/// `longest` pixels on its longest side.
pub fn read_still(name: &str, bytes: &[u8], longest: u32) -> Result<Still, ImportError> {
    if bytes.len() > MAX_STILL_BYTES {
        return Err(bad(name, "the file is too large for a picture"));
    }
    let ending = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let picture = match ending.as_str() {
        "png" => decode_png(name, bytes)?,
        "jpg" | "jpeg" => decode_jpeg(name, bytes)?,
        "gif" => decode_gif(name, bytes)?
            .into_iter()
            .next()
            .ok_or_else(|| bad(name, "the GIF has no frame"))?,
        other if STILL_EXTENSIONS.contains(&other) => {
            let format = image::ImageFormat::from_extension(other)
                .ok_or_else(|| bad(name, "not a kind of picture Flare reads"))?;
            decode_raster(name, bytes, format)?
        }
        _ => return Err(bad(name, "not a kind of picture Flare reads")),
    };
    Ok(shrink(picture, longest.max(1)))
}

/// `picture` made no more than `longest` on a side, each kept pixel the average of the ones it
/// covers (weighted by how opaque they are, so a see-through edge does not darken).
fn shrink(picture: Picture, longest: u32) -> Still {
    let (w, h) = (picture.width, picture.height);
    let k = (f64::from(longest) / f64::from(w.max(h))).min(1.0);
    let kept = [
        ((f64::from(w) * k).round() as u32).max(1),
        ((f64::from(h) * k).round() as u32).max(1),
    ];
    if kept == [w, h] {
        return Still {
            width: w,
            height: h,
            kept,
            rgba: picture.rgba,
        };
    }
    let mut rgba = Vec::with_capacity(kept[0] as usize * kept[1] as usize * 4);
    for y in 0..kept[1] {
        let (y0, y1) = span(y, kept[1], h);
        for x in 0..kept[0] {
            let (x0, x1) = span(x, kept[0], w);
            let mut sum = [0u64; 4];
            for sy in y0..y1 {
                let row = sy as usize * w as usize;
                for sx in x0..x1 {
                    let i = (row + sx as usize) * 4;
                    let a = u64::from(picture.rgba[i + 3]);
                    for (c, total) in sum.iter_mut().take(3).enumerate() {
                        *total += u64::from(picture.rgba[i + c]) * a;
                    }
                    sum[3] += a;
                }
            }
            let count = u64::from((x1 - x0) * (y1 - y0)).max(1);
            let alpha = sum[3];
            for c in sum.iter().take(3) {
                rgba.push(c.checked_div(alpha).unwrap_or(0) as u8);
            }
            rgba.push((alpha / count) as u8);
        }
    }
    Still {
        width: w,
        height: h,
        kept,
        rgba,
    }
}

/// The source pixels kept pixel `i` of `kept` covers, of `full`.
fn span(i: u32, kept: u32, full: u32) -> (u32, u32) {
    let start = (u64::from(i) * u64::from(full) / u64::from(kept)) as u32;
    let end = ((u64::from(i) + 1) * u64::from(full) / u64::from(kept)) as u32;
    (start, end.max(start + 1).min(full))
}
