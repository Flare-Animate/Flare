//! What can go wrong when ffmpeg is asked for something.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FfmpegError {
    /// ffmpeg (and its `ffprobe`) is not on PATH.
    #[error("ffmpeg is not on this computer: install it from ffmpeg.org and put it on the PATH")]
    NotInstalled,
    /// ffmpeg ran and failed; `detail` is the end of what it said.
    #[error("ffmpeg could not {action}: {detail}")]
    Failed {
        action: &'static str,
        detail: String,
    },
    /// The run went past its time limit and was stopped.
    #[error("ffmpeg took longer than {0} seconds and was stopped")]
    TimedOut(u64),
    /// The caller stopped the reading part way.
    #[error("the reading was stopped")]
    Stopped,
    /// The request is outside what is allowed (a size, a count, or a path ffmpeg must not read).
    #[error("{0}")]
    Refused(String),
    /// A pipe or the file system failed.
    #[error("ffmpeg could not be run: {0}")]
    Io(#[from] std::io::Error),
}
