//! The bytes of a picture file, chosen by the ending its name has: a still format for the frame
//! on show, or an animation of the whole scene. Every surface names its files this way, so the
//! desktop and the browser write the same bytes for the same name.

use flare_document::Document;
use flare_render::{Backdrop, StillFormat};

use crate::{apng_bytes, gif_bytes, psd_export, still_bytes};

/// Every ending a still picture of the frame on show is written with: the still formats and PSD
/// (one flattened picture, not layered).
pub const STILL_ENDINGS: [&str; 15] = [
    "png", "jpg", "jpeg", "bmp", "tga", "tif", "tiff", "webp", "qoi", "pnm", "pam", "ico", "exr",
    "hdr", "psd",
];

/// Whether `ending` names a still picture of the frame on show (see [`STILL_ENDINGS`]).
pub fn is_still_ending(ending: &str) -> bool {
    STILL_ENDINGS.iter().any(|e| e.eq_ignore_ascii_case(ending))
}

/// The file `ending` names, as bytes: a still (see [`STILL_ENDINGS`]) of `frame`, or the animation
/// of the whole first scene as a GIF (`gif`) or an APNG (`apng`). Over a transparent `backdrop` a format with no alpha is
/// refused before anything is drawn. Any other ending is refused by name, and the movie endings
/// (`mp4`, `mov`, the ffmpeg containers) are not here: they need a video encoder, which only the
/// desktop has.
pub fn picture_bytes(
    document: &Document,
    frame: u32,
    ending: &str,
    backdrop: Backdrop,
) -> Result<Vec<u8>, String> {
    let ending = ending.to_ascii_lowercase();
    if ending == "psd" {
        return psd_export(document, frame, backdrop);
    }
    if let Some(format) = StillFormat::from_extension(&ending) {
        return still_bytes(document, frame, format, backdrop);
    }
    match ending.as_str() {
        "gif" => gif_bytes(document, backdrop),
        "apng" => apng_bytes(document, backdrop),
        other => Err(format!(
            "A picture is written as {}, .gif or .apng here, not .{other}",
            STILL_ENDINGS.map(|e| format!(".{e}")).join(", ")
        )),
    }
}
