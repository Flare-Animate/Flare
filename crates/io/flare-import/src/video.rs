//! Movies (MP4, MOV, AVI, Matroska, WebM, MPEG, Flash video) as projects: each picture is a
//! one-frame bitmap keyframe, at the movie's own rate. ffmpeg reads the pictures (`flare-ffmpeg`);
//! the sound is not imported. A movie is read from its file, since ffmpeg reads files.

use std::path::Path;

use flare_core::FrameRate;
use flare_ffmpeg::FfmpegError;

use crate::bitmap::{MAX_IMAGE_FRAMES, Picture, bad, build};
use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

/// The longest side a movie's pictures are read at, in pixels; bigger ones are scaled down.
const MAX_SIDE: u32 = 1920;

/// Imports the movie at `path`.
pub(crate) fn import_path(path: &Path) -> Result<Imported, ImportError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("movie")
        .to_owned();
    let limit = u32::try_from(MAX_IMAGE_FRAMES).unwrap_or(u32::MAX);
    let clip =
        flare_ffmpeg::decode(path, limit, MAX_SIDE).map_err(|error| bad(&name, describe(error)))?;
    let rate = FrameRate::new(clip.rate.0, clip.rate.1).map_err(|e| bad(&name, e))?;
    let (width, height, truncated, frames) = (
        clip.width,
        clip.height,
        clip.may_be_truncated,
        clip.frames.len(),
    );
    let pictures: Vec<Picture> = clip
        .frames
        .into_iter()
        .map(|rgba| Picture {
            width,
            height,
            rgba,
            delay_cs: 0,
        })
        .collect();
    let holds = vec![1_u32; pictures.len()];
    let mut imported = build(&name, pictures, &holds, rate)?;
    imported.diagnostics.push(Diagnostic {
        severity: Severity::Info,
        location: name.clone(),
        message: "the sound is not imported".into(),
    });
    if truncated {
        imported.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            location: name,
            message: format!("imported the first {frames} frames; the movie may have more"),
        });
    }
    Ok(imported)
}

/// What a failure to read a movie means to the person who opened it.
fn describe(error: FfmpegError) -> String {
    match error {
        FfmpegError::NotInstalled => "reading a movie needs ffmpeg, which is not on this computer: install it from ffmpeg.org and put it on the PATH".to_owned(),
        other => other.to_string(),
    }
}
