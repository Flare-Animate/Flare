//! A video brought into an open document to draw over: ffprobe says what it is
//! ([`probe_video`]), `flare_tools::video::plan` says what reading it would make, and
//! [`read_video`] reads exactly that through ffmpeg, a picture at a time, so a caller can show
//! how far it has got and stop it. [`import_video`] reads and places in one call, for the
//! command line and the MCP server; the desktop app reads on a worker and places the result.

use std::path::Path;

use flare_core::{FrameRate, SceneId};
use flare_document::Document;
use flare_ffmpeg::{FfmpegError, Sampling, Tools};
use flare_tools::video::{Placed, Plan, Source, VideoRefusal};
use thiserror::Error;

/// The endings of the movie files ffmpeg is asked to read, by File > Open and Import Video.
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "mpeg", "mpg", "flv"];

/// Why a video could not be brought in.
#[derive(Debug, Error)]
pub enum VideoImportError {
    #[error(
        "importing a video needs ffmpeg, which is not on this computer: install it from ffmpeg.org and put it on the PATH"
    )]
    NoFfmpeg,
    /// The caller stopped the reading.
    #[error("the video import was stopped")]
    Stopped,
    /// ffmpeg could not read the file, and what it said.
    #[error("cannot read the video: {0}")]
    Unreadable(String),
    #[error(transparent)]
    Refused(#[from] VideoRefusal),
}

impl From<FfmpegError> for VideoImportError {
    fn from(error: FfmpegError) -> Self {
        match error {
            FfmpegError::NotInstalled => Self::NoFfmpeg,
            FfmpegError::Stopped => Self::Stopped,
            other => Self::Unreadable(other.to_string()),
        }
    }
}

/// What the movie at `path` is, through the ffprobe on PATH.
pub fn probe_video(path: &Path) -> Result<Source, VideoImportError> {
    probe_video_with(&flare_ffmpeg::locate()?, path)
}

/// [`probe_video`] with the programs given.
pub fn probe_video_with(tools: &Tools, path: &Path) -> Result<Source, VideoImportError> {
    let probe = flare_ffmpeg::probe_with(tools, path)?;
    let rate = FrameRate::new(probe.rate.0, probe.rate.1)
        .map_err(|_| VideoImportError::Unreadable("it has no frame rate".to_owned()))?;
    Ok(Source {
        width: probe.width,
        height: probe.height,
        rate,
        seconds: probe.seconds,
    })
}

/// The pictures `plan` asks for, read from the movie at `path` by the ffmpeg on PATH. `progress`
/// hears how many have arrived after each one and returns false to stop. A plan that does not fit
/// is refused before anything is read.
pub fn read_video(
    path: &Path,
    plan: &Plan,
    progress: impl FnMut(u32) -> bool,
) -> Result<Vec<Vec<u8>>, VideoImportError> {
    read_video_with(&flare_ffmpeg::locate()?, path, plan, progress)
}

/// [`read_video`] with the programs given.
pub fn read_video_with(
    tools: &Tools,
    path: &Path,
    plan: &Plan,
    mut progress: impl FnMut(u32) -> bool,
) -> Result<Vec<Vec<u8>>, VideoImportError> {
    if let Some(why) = plan.too_large {
        return Err(VideoRefusal::TooLarge(why).into());
    }
    let rate = plan.settings.rate;
    let sampling = Sampling {
        rate: (rate.numerator(), rate.denominator()),
        size: plan.size,
        max_frames: plan.frames,
    };
    let mut pictures = Vec::with_capacity(usize::try_from(plan.frames).unwrap_or(0).min(4096));
    flare_ffmpeg::read_frames_with(tools, path, sampling, |picture| {
        pictures.push(picture);
        progress(u32::try_from(pictures.len()).unwrap_or(u32::MAX))
    })?;
    Ok(pictures)
}

/// Reads the movie at `path` as `plan` says and places it as a layer called `name` at the
/// bottom of `scene` from `frame`: the commands to apply, as one undo step.
pub fn import_video(
    document: &Document,
    scene: SceneId,
    frame: u32,
    name: &str,
    path: &Path,
    plan: &Plan,
) -> Result<Placed, VideoImportError> {
    import_video_with(
        &flare_ffmpeg::locate()?,
        document,
        scene,
        frame,
        name,
        path,
        plan,
    )
}

/// [`import_video`] with the programs given.
pub fn import_video_with(
    tools: &Tools,
    document: &Document,
    scene: SceneId,
    frame: u32,
    name: &str,
    path: &Path,
    plan: &Plan,
) -> Result<Placed, VideoImportError> {
    let pictures = read_video_with(tools, path, plan, |_| true)?;
    Ok(flare_tools::video::place(
        document, scene, frame, name, plan, pictures,
    )?)
}
